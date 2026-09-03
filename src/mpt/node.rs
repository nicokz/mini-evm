use crate::crypto::keccak256;

pub type Nibbles = Vec<u8>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Null,
    Leaf {
        key_path: Nibbles,
        value: Vec<u8>,
    },
    Extension {
        key_path: Nibbles,
        child: Box<Node>,
    },
    Branch {
        children: [Box<Node>; 16],
        value: Option<Vec<u8>>,
    },
}

pub fn bytes_to_nibbles(bytes: &[u8]) -> Nibbles {
    bytes
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .collect()
}

fn compact_encode(path: &[u8], leaf: bool) -> Vec<u8> {
    let odd = path.len() % 2 == 1;
    let flags = if leaf { 2 } else { 0 } + if odd { 1 } else { 0 };
    let mut encoded = Vec::with_capacity((path.len() + 2) / 2);
    let mut index = 0;
    if odd {
        encoded.push((flags << 4) | path[0]);
        index = 1;
    } else {
        encoded.push(flags << 4);
    }
    while index < path.len() {
        encoded.push((path[index] << 4) | path[index + 1]);
        index += 2;
    }
    encoded
}

pub fn compact_decode(encoded: &[u8]) -> (Nibbles, bool) {
    if encoded.is_empty() {
        return (Vec::new(), false);
    }
    let flags = encoded[0] >> 4;
    let odd = flags & 1 == 1;
    let leaf = flags & 2 == 2;
    let mut path = Vec::new();
    if odd {
        path.push(encoded[0] & 0x0f);
    }
    for byte in &encoded[1..] {
        path.push(byte >> 4);
        path.push(byte & 0x0f);
    }
    (path, leaf)
}

fn rlp_bytes(bytes: &[u8]) -> Vec<u8> {
    if bytes.len() == 1 && bytes[0] < 0x80 {
        return bytes.to_vec();
    }
    if bytes.len() <= 55 {
        let mut encoded = vec![0x80 + bytes.len() as u8];
        encoded.extend_from_slice(bytes);
        encoded
    } else {
        let length = bytes.len().to_be_bytes();
        let first = length
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(length.len() - 1);
        let length_bytes = &length[first..];
        let mut encoded = vec![0xb7 + length_bytes.len() as u8];
        encoded.extend_from_slice(length_bytes);
        encoded.extend_from_slice(bytes);
        encoded
    }
}

fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload_len: usize = items.iter().map(Vec::len).sum();
    let payload: Vec<u8> = items.iter().flatten().copied().collect();
    if payload_len <= 55 {
        let mut encoded = vec![0xc0 + payload_len as u8];
        encoded.extend(payload);
        encoded
    } else {
        let length = payload_len.to_be_bytes();
        let first = length
            .iter()
            .position(|byte| *byte != 0)
            .unwrap_or(length.len() - 1);
        let length_bytes = &length[first..];
        let mut encoded = vec![0xf7 + length_bytes.len() as u8];
        encoded.extend_from_slice(length_bytes);
        encoded.extend(payload);
        encoded
    }
}

pub fn node_reference(node: &Node) -> Vec<u8> {
    let encoded = rlp_encode(node);
    if encoded.len() >= 32 {
        keccak256(&encoded).to_vec()
    } else {
        encoded
    }
}

pub fn rlp_encode(node: &Node) -> Vec<u8> {
    match node {
        Node::Null => rlp_bytes(&[]),
        Node::Leaf { key_path, value } => {
            rlp_list(&[rlp_bytes(&compact_encode(key_path, true)), rlp_bytes(value)])
        }
        Node::Extension { key_path, child } => rlp_list(&[
            rlp_bytes(&compact_encode(key_path, false)),
            node_reference(child),
        ]),
        Node::Branch { children, value } => {
            let mut items = children
                .iter()
                .map(|child| node_reference(child))
                .collect::<Vec<_>>();
            items.push(rlp_bytes(value.as_deref().unwrap_or(&[])));
            rlp_list(&items)
        }
    }
}
