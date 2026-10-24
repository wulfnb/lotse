import {test} from 'node:test';
import assert from 'node:assert/strict';
import {QualityController, ViewportController} from '../docker/viewer/controls.js';

test('Auto degrades only sustained backlog and recovers only with completed clear frames',()=>{
  const q=new QualityController();
  const sample=(pending=0,oldestMs=0,completed=0,frames=0,slow=false)=>q.sample({pending,oldestMs,completed,frames},slow);
  assert.equal(q.sample(),6);
  assert.equal(q.sample({},true),2);
  assert.equal(sample(1,200),6);
  assert.equal(sample(1,200),2);
  for(let n=0;n<20;n++) assert.equal(sample(),2); // Idle is not recovery evidence.
  for(let n=1;n<=10;n++) sample(0,0,n);
  assert.equal(q.level,6);
  for(let n=11;n<=20;n++) sample(0,0,n);
  assert.equal(q.level,9);
  sample(1,0,20,2); sample(1,0,20,2); assert.equal(q.level,2);
  q.reset(true); assert.equal(q.level,2);
  sample(24); sample(24); assert.equal(q.level,2);
  q.reset(false); assert.equal(sample(0,0,1,0,true),2);
});

test('Viewer gestures preserve the centroid, bound zoom, and pan without remote input',()=>{
  const changes=[];
  const client={viewportMetrics:{x:0,y:0,scale:1,width:1000,height:800},setViewport(...args){changes.push(args);}};
  const classes=[];
  const screen={clientWidth:500,clientHeight:400,classList:{toggle(...a){classes.push(a);}}};
  const v=new ViewportController(client,screen);
  v.enable(true);assert.equal(client.viewOnly,true);
  v.reset(); assert.deepEqual(changes.pop(),[.5,0,0]);
  v.start([{x:100,y:100}]);v.move([{x:80,y:70}]);assert.deepEqual(changes.pop(),[1,20,30]);
  v.start([{x:100,y:100},{x:200,y:100}]);v.move([{x:50,y:100},{x:250,y:100}]);assert.deepEqual(changes.pop(),[2,75,50]);
  v.move([{x:150,y:100},{x:150,y:100}]);assert.equal(changes.pop()[0],.5);
  v.start([{x:10,y:10},{x:10,y:10}]);v.move([{x:20,y:20},{x:20,y:20}]);assert.equal(changes.pop()[0],1);
  v.move([]);v.move([]);v.move([{x:0,y:0}]);assert.equal(changes.length,0);
  v.enable(false);assert.equal(client.viewOnly,false);assert.equal(classes.length,2);
  client.viewportMetrics={};v.reset();v.move([{x:1,y:1}]);assert.equal(changes.length,0);
});
