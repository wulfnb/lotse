use blitz_dom::{BaseDocument, NodeId};
use html5ever::{LocalName, QualName, ns};
use rquickjs::{Coerced, Context, FromJs, Function, Runtime};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

/// Rust owns the only DOM. The VM sees checked generational IDs, never pointers.
pub(crate) struct ScriptHost {
    context: Context,
    runtime: Runtime,
    deadline: Arc<Mutex<Instant>>,
    budget: std::time::Duration,
}
impl ScriptHost {
    pub fn new(
        doc: Rc<RefCell<BaseDocument>>,
        limits: &crate::Limits,
        navigate: Rc<RefCell<Vec<url::Url>>>,
        initial_bytes: usize,
    ) -> Result<Self, String> {
        let runtime = Runtime::new().map_err(|e| e.to_string())?;
        runtime.set_memory_limit(limits.js_heap_bytes);
        runtime.set_gc_threshold(limits.js_heap_bytes / 4);
        runtime.set_max_stack_size(512 * 1024);
        let deadline = Arc::new(Mutex::new(Instant::now() + limits.script_time));
        let check = deadline.clone();
        runtime.set_interrupt_handler(Some(Box::new(move || {
            Instant::now()
                >= *check
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
        })));
        let context = Context::full(&runtime).map_err(|e| e.to_string())?;
        let max_nodes = limits.dom_nodes;
        let allocated = Rc::new(std::cell::Cell::new(
            blitz_dom::traversal::TreeTraverser::new(&doc.borrow()).count(),
        ));
        let start = Instant::now();
        let used = std::cell::Cell::new(initial_bytes);
        let max_dom_bytes = limits.dom_text_bytes;
        context.with(|ctx| {
            let host=Function::new(ctx.clone(), move |message: String| -> String {
                let result=(|| -> Result<Value,String> {
                    let args: Vec<Value>=serde_json::from_str(&message).map_err(|e|e.to_string())?;
                    let text=|index:usize|args.get(index).and_then(Value::as_str).unwrap_or_default();
                    let charge=match text(0){"setText"=>text(2).len()+32,"setAttr"=>text(2).len()+text(3).len()+32,"create"|"createText"=>text(1).len()+32,_=>0};
                    let next=used.get().saturating_add(charge);
                    if next>max_dom_bytes{return Err("DOM text allocation budget exceeded".into());}used.set(next);
                    let id=|index:usize|text(index).parse::<u64>().map(NodeId::from_u64).map_err(|_|"Invalid node identity".to_string());
                    let mut doc=doc.try_borrow_mut().map_err(|_|"DOM is currently busy".to_string())?;
                    let node_id=|| -> Result<NodeId,String> {let id=id(1)?;if doc.get_node(id).is_none(){Err("Node no longer exists".into())}else{Ok(id)}};
                    let value=match text(0) {
                        "root"=>json!(doc.root_node().id.as_u64().to_string()),
                        "query"=>json!(doc.query_selector(text(1)).map_err(|_|"Invalid selector")?.map(|id|id.as_u64().to_string())),
                        "queryAll"=>json!(doc.query_selector_all(text(1)).map_err(|_|"Invalid selector")?.iter().map(|id|id.as_u64().to_string()).collect::<Vec<_>>()),
                        "text"=>json!(doc.get_node(node_id()?).unwrap().text_content()),
                        "tag"=>json!(doc.get_node(node_id()?).unwrap().element_data().map(|e|e.name.local.to_string().to_uppercase())),
                        "children"=>json!(doc.get_node(node_id()?).unwrap().children.iter().map(|id|id.as_u64().to_string()).collect::<Vec<_>>()),
                        "parent"=>json!(doc.get_node(node_id()?).unwrap().parent.map(|id|id.as_u64().to_string())),
                        "rect"=>{
                            let id=node_id()?;doc.resolve(start.elapsed().as_secs_f64());
                            let node=doc.get_node(id).unwrap();let p=node.absolute_position(0.0,0.0);let scroll=doc.viewport_scroll();let size=node.final_layout().size;
                            let x=p.x-scroll.x as f32;let y=p.y-scroll.y as f32;
                            json!({"x":x,"y":y,"width":size.width,"height":size.height,"left":x,"top":y,"right":x+size.width,"bottom":y+size.height})
                        },
                        "attr"=>json!(doc.get_node(node_id()?).unwrap().element_data().and_then(|e|e.attr(LocalName::from(text(2)))).map(str::to_owned)),
                        "setText"=>{
                            let id=node_id()?;
                            if text(2).len()>1024*1024 || allocated.get()>=max_nodes {return Err("DOM text or node budget exceeded".into());}
                            allocated.set(allocated.get()+1);
                            if matches!(doc.get_node(id).unwrap().data,blitz_dom::node::NodeData::Text(_)) {doc.mutate().set_node_text(id,text(2));}
                            else {let mut mutation=doc.mutate();let child=mutation.create_text_node(text(2));mutation.replace_children(id,&[child]);}Value::Null
                        },
                        "setAttr"=>{
                            let id=node_id()?;
                            if text(3).len()>1024*1024 {return Err("DOM attribute budget exceeded".into());}
                            doc.mutate().set_attribute(id,QualName::new(None,ns!(),LocalName::from(text(2))),text(3));Value::Null
                        },
                        "removeAttr"=>{let id=node_id()?;doc.mutate().clear_attribute(id,QualName::new(None,ns!(),LocalName::from(text(2))));Value::Null},
                        "create"|"createText"=>{
                            if allocated.get()>=max_nodes || text(1).len()>1024*1024 {return Err("DOM node budget exceeded".into());}
                            allocated.set(allocated.get()+1);
                            let id=if text(0)=="create" {doc.mutate().create_element(QualName::new(None,ns!(html),LocalName::from(text(1))),vec![])} else {doc.mutate().create_text_node(text(1))};
                            json!(id.as_u64().to_string())
                        },
                        "append"=>{
                            let parent=node_id()?;let child=id(2)?;
                            if doc.get_node(child).is_none(){return Err("Node no longer exists".into());}
                            // Reject cycles, including putting a node below itself.
                            let mut ancestor=Some(parent);
                            while let Some(id)=ancestor {if id==child{return Err("Cannot create a DOM cycle".into());}ancestor=doc.get_node(id).and_then(|n|n.parent);}
                            doc.mutate().append_children(parent,&[child]);json!(child.as_u64().to_string())
                        },
                        "remove"=>{let id=node_id()?;doc.mutate().remove_node(id);Value::Null},
                        "url"=>json!(doc.url().as_str()),
                        "navigate"=>{
                            let target=doc.url().join(text(1)).map_err(|e|e.to_string())?;
                            if !matches!(target.scheme(),"http"|"https") && !(target.scheme()=="file" && doc.url().scheme()=="file") {return Err("Navigation scheme is not allowed".into());}
                            if navigate.borrow().len()>=8{return Err("Too many pending navigations".into());}
                            navigate.borrow_mut().push(target);Value::Null
                        },
                        "now"=>json!(start.elapsed().as_secs_f64()*1000.0),
                        "log"=>{eprintln!("Wega JavaScript: {}",text(1).chars().take(4096).collect::<String>());Value::Null},
                        _=>return Err("This Web API is not available; use Servo for this page".into()),
                    };Ok(value)
                })();
                match result {Ok(value)=>json!({"value":value}),Err(error)=>json!({"error":error})}.to_string()
            }).map_err(|e|e.to_string())?;
            ctx.globals().set("__wega_host",host).map_err(|e|e.to_string())
        })?;
        let host = Self {
            context,
            runtime,
            deadline,
            budget: limits.script_time,
        };
        host.run(include_str!("dom.js"))?;
        Ok(host)
    }
    fn arm(&self) {
        *self
            .deadline
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now() + self.budget;
    }
    pub fn run(&self, source: &str) -> Result<String, String> {
        self.arm();
        self.context.with(|ctx| {
            ctx.eval::<rquickjs::Value, _>(source)
                .map(|v| format!("{v:?}"))
                .map_err(|e| {
                    let detail = if e.is_exception() {
                        Coerced::<String>::from_js(&ctx, ctx.catch())
                            .map(|v| v.0)
                            .unwrap_or_else(|_| "Script was interrupted".into())
                    } else {
                        e.to_string()
                    };
                    format!("JavaScript failed or exceeded its limit: {detail}")
                })
        })
    }
    pub fn dispatch(&self, event: &str, id: NodeId) -> Result<bool, String> {
        self.arm();
        self.context.with(|ctx| {
            ctx.eval::<bool, _>(format!(
                "__wega_dispatch({}, {})",
                json!(event),
                json!(id.as_u64().to_string())
            ))
            .map_err(|e| e.to_string())
        })
    }
    pub fn evaluate_json(&self, source: &str) -> Result<String, String> {
        self.arm();
        self.context.with(|ctx| {
            ctx.eval::<String, _>(format!(
                "JSON.stringify((0,eval)({})) ?? 'null'",
                json!(source)
            ))
            .map_err(|e| e.to_string())
        })
    }
    pub fn tick(&self) -> Result<(), String> {
        self.run("__wega_timers()")?;
        self.arm();
        for _ in 0..32 {
            if Instant::now()
                >= *self
                    .deadline
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
            {
                break;
            }
            if !self
                .runtime
                .execute_pending_job()
                .map_err(|_| "Promise job failed or exceeded its limit")?
            {
                break;
            }
        }
        if self.runtime.is_job_pending() {
            return Err("Microtask budget exceeded".into());
        }
        Ok(())
    }
    pub fn next_timer(&self) -> Option<std::time::Duration> {
        self.arm();
        self.context
            .with(|ctx| ctx.eval::<f64, _>("__wega_next_timer()").ok())
            .filter(|n| n.is_finite())
            .map(|n| std::time::Duration::from_secs_f64((n.max(0.0) / 1000.0).min(60.0)))
    }
    pub fn heap_bytes(&self) -> usize {
        self.runtime.memory_usage().memory_used_size.max(0) as usize
    }
}
