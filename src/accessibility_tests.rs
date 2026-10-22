use super::*;
#[test]
fn servo_subtree_and_actions_preserve_semantics_across_versions() {
    let mut node = servo::accesskit::Node::new(servo::accesskit::Role::Button);
    node.set_label("Submit form");
    node.add_action(servo::accesskit::Action::Click);
    let update = servo::accesskit::TreeUpdate {
        nodes: vec![(servo::accesskit::NodeId(1), node)],
        tree: Some(servo::accesskit::Tree::new(servo::accesskit::NodeId(1))),
        tree_id: servo::accesskit::TreeId::ROOT,
        focus: servo::accesskit::NodeId(1),
    };
    let converted = tree(update).unwrap();
    assert_eq!(converted.nodes[0].1.label(), Some("Submit form"));
    assert!(
        converted.nodes[0]
            .1
            .supports_action(accesskit::Action::Click)
    );
    let request = accesskit::ActionRequest {
        action: accesskit::Action::Click,
        target_tree: accesskit::TreeId::ROOT,
        target_node: accesskit::NodeId(1),
        data: None,
    };
    let converted: servo::accesskit::ActionRequest =
        serde_json::from_value(serde_json::to_value(request).unwrap()).unwrap();
    assert_eq!(converted.action, servo::accesskit::Action::Click);
}
