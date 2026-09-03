use super::node::{Node, bytes_to_nibbles, node_reference, rlp_encode};
use crate::crypto::keccak256;

#[derive(Debug, Clone)]
pub struct MerklePatriciaTrie {
    pub root: Node,
}

impl Default for MerklePatriciaTrie {
    fn default() -> Self {
        Self::new()
    }
}

impl MerklePatriciaTrie {
    pub fn new() -> Self {
        Self { root: Node::Null }
    }

    pub fn insert(&mut self, key: &[u8], value: Vec<u8>) {
        let path = bytes_to_nibbles(key);
        self.root = insert_node(std::mem::replace(&mut self.root, Node::Null), &path, value);
    }

    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        get_node(&self.root, &bytes_to_nibbles(key))
    }

    pub fn root_hash(&self) -> [u8; 32] {
        if matches!(self.root, Node::Null) {
            return keccak256(&[0x80]);
        }
        keccak256(&rlp_encode(&self.root))
    }
}

fn empty_children() -> [Box<Node>; 16] {
    std::array::from_fn(|_| Box::new(Node::Null))
}

fn common_prefix(left: &[u8], right: &[u8]) -> usize {
    left.iter().zip(right).take_while(|(a, b)| a == b).count()
}

fn child_for(path: &[u8], value: Vec<u8>) -> Node {
    Node::Leaf {
        key_path: path.to_vec(),
        value,
    }
}

#[allow(clippy::replace_box)]
fn insert_node(node: Node, path: &[u8], value: Vec<u8>) -> Node {
    match node {
        Node::Null => child_for(path, value),
        Node::Leaf {
            key_path,
            value: old_value,
        } => {
            let common = common_prefix(&key_path, path);
            if common == key_path.len() && common == path.len() {
                return child_for(path, value);
            }
            let mut children = empty_children();
            let old_remaining = &key_path[common..];
            if old_remaining.is_empty() {
                let branch_value = Some(old_value);
                if path[common..].is_empty() {
                    return Node::Branch {
                        children,
                        value: Some(value),
                    };
                }
                children[path[common] as usize] = Box::new(child_for(&path[common + 1..], value));
                let branch = Node::Branch {
                    children,
                    value: branch_value,
                };
                return if common == 0 {
                    branch
                } else {
                    Node::Extension {
                        key_path: path[..common].to_vec(),
                        child: Box::new(branch),
                    }
                };
            }
            children[old_remaining[0] as usize] =
                Box::new(child_for(&old_remaining[1..], old_value));
            let new_remaining = &path[common..];
            let branch_value = if new_remaining.is_empty() {
                Some(value.clone())
            } else {
                None
            };
            if !new_remaining.is_empty() {
                children[new_remaining[0] as usize] =
                    Box::new(child_for(&new_remaining[1..], value));
            }
            let branch = Node::Branch {
                children,
                value: branch_value,
            };
            if common == 0 {
                branch
            } else {
                Node::Extension {
                    key_path: path[..common].to_vec(),
                    child: Box::new(branch),
                }
            }
        }
        Node::Extension { key_path, child } => {
            let common = common_prefix(&key_path, path);
            if common == key_path.len() {
                return if common == path.len() {
                    Node::Extension {
                        key_path,
                        child: Box::new(insert_node(*child, &[], value)),
                    }
                } else {
                    Node::Extension {
                        key_path,
                        child: Box::new(insert_node(*child, &path[common..], value)),
                    }
                };
            }
            let mut children = empty_children();
            let old_remaining = &key_path[common..];
            let old_child = if old_remaining.len() == 1 {
                *child
            } else {
                Node::Extension {
                    key_path: old_remaining[1..].to_vec(),
                    child,
                }
            };
            children[old_remaining[0] as usize] = Box::new(old_child);
            let new_remaining = &path[common..];
            let branch_value = if new_remaining.is_empty() {
                Some(value.clone())
            } else {
                None
            };
            if !new_remaining.is_empty() {
                children[new_remaining[0] as usize] =
                    Box::new(child_for(&new_remaining[1..], value));
            }
            let branch = Node::Branch {
                children,
                value: branch_value,
            };
            if common == 0 {
                branch
            } else {
                Node::Extension {
                    key_path: path[..common].to_vec(),
                    child: Box::new(branch),
                }
            }
        }
        Node::Branch {
            mut children,
            value: branch_value,
        } => {
            if path.is_empty() {
                Node::Branch {
                    children,
                    value: Some(value),
                }
            } else {
                let index = path[0] as usize;
                let child = std::mem::replace(&mut children[index], Box::new(Node::Null));
                children[index] = Box::new(insert_node(*child, &path[1..], value));
                Node::Branch {
                    children,
                    value: branch_value,
                }
            }
        }
    }
}

fn get_node(node: &Node, path: &[u8]) -> Option<Vec<u8>> {
    match node {
        Node::Null => None,
        Node::Leaf { key_path, value } => (key_path == path).then(|| value.clone()),
        Node::Extension { key_path, child } => path
            .starts_with(key_path)
            .then(|| get_node(child, &path[key_path.len()..]))
            .flatten(),
        Node::Branch { children, value } => {
            if path.is_empty() {
                value.clone()
            } else {
                get_node(&children[path[0] as usize], &path[1..])
            }
        }
    }
}

#[allow(dead_code)]
pub(crate) fn node_ref(node: &Node) -> Vec<u8> {
    node_reference(node)
}
