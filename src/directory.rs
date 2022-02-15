use std::collections::HashMap;
use std::fmt::Display;
use std::fs;

use color_eyre::{eyre::eyre, Result};
use indextree::{Arena, NodeId};

const DIR: &str = "data/xochitl";

fn extract_field<'a>(metadata: &'a str, pattern: &str) -> Option<&'a str> {
    let a = metadata.find(pattern)? + pattern.len();
    let b = a + metadata[a..].find('\"').unwrap();

    Some(&metadata[a..b])
}

fn parent(metadata: &str) -> Option<&str> {
    extract_field(metadata, "\"parent\": \"")
}

fn name(metadata: &str) -> Option<&str> {
    extract_field(metadata, "\"visibleName\": \"")
}

fn is_folder(metadata: &str) -> bool {
    let doc_type = extract_field(metadata, "\"type\": \"").unwrap();
    match doc_type {
        "DocumentType" => false,
        "CollectionType" => true,
        _t => panic!("unexpected document type: {_t}"),
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct Uuid(String);

impl std::fmt::Display for Uuid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::convert::From<&str> for Uuid {
    fn from(s: &str) -> Self {
        Uuid(s.to_owned())
    }
}

pub struct Tree {
    arena: Arena<()>,
    node: HashMap<Uuid, NodeId>,
    name: HashMap<NodeId, String>,
    files: HashMap<NodeId, Vec<Uuid>>,
}

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node_id = self.root(Uuid("".to_owned()));
        self.print(*node_id, 0, f)?;
        Ok(())
    }
}
impl Tree {
    fn add_root(&mut self, uuid: Uuid, name: impl Into<String>) {
        let node_id = self.arena.new_node(());
        self.name.insert(node_id, name.into());
        self.node.insert(uuid, node_id);
    }

    pub fn new() -> Self {
        let mut tree = Self {
            arena: Arena::new(),
            node: HashMap::new(),
            name: HashMap::new(),
            files: HashMap::new(),
        };

        tree.add_root(Uuid("trash".to_owned()), "trash");
        tree.add_root(Uuid("".to_owned()), "");
        tree
    }

    pub fn print(
        &self,
        node: NodeId,
        indent: usize,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let ident_str: String = std::iter::once(' ').cycle().take(indent * 4).collect();
        let node_name = self.name.get(&node).unwrap();
        match indent {
            0 => writeln!(f, "    {node_name}")?,
            _ => writeln!(f, "{ident_str}|-- {node_name}")?,
        }
        if let Some(files) = self.files.get(&node) {
            for file in files {
                writeln!(f, "{ident_str}    |-- {file}")?;
            }
        }

        for child in node.children(&self.arena) {
            self.print(child, indent + 1, f)?;
        }
        Ok(())
    }

    pub fn root(&self, uuid: Uuid) -> &NodeId {
        self.node.get(&uuid).unwrap()
    }

    pub fn children(&self, path: String) -> Result<(Vec<Uuid>, Vec<String>)> {
        let mut files = Vec::new();
        let mut folders = Vec::new();

        let mut node = *self.root(Uuid("".to_owned()));
        if !path.is_empty() {
            for comp in path.split('/') {
                node = node
                    .children(&self.arena)
                    .find(|n| {
                        let name = self.name.get(n).unwrap();
                        name == comp
                    })
                    .ok_or_else(|| eyre!("path incorrect, failed to find component: {comp}"))?;
            }
        }

        for folder in node.descendants(&self.arena) {
            if let Some(content) = self.files.get(&folder) {
                files.extend_from_slice(content);
            }
            let folder = self.name.get(&folder).unwrap();
            folders.push(folder.to_owned());
        }
        Ok((files, folders))
    }

    pub fn add_file(&mut self, uuid: Uuid, parent_uuid: Uuid) {
        let parent_node = match self.node.get(&parent_uuid) {
            Some(n) => *n,
            None => {
                let parent_node = self.arena.new_node(());
                self.node.insert(parent_uuid, parent_node);
                parent_node
            }
        };
        match self.files.get_mut(&parent_node) {
            Some(list) => list.push(uuid),
            None => {
                self.files.insert(parent_node, vec![uuid]);
            }
        }
    }

    pub fn add_folder(&mut self, uuid: Uuid, parent_uuid: Uuid, name: String) {
        let node_id = match self.node.get(&uuid) {
            Some(node) => *node,
            None => {
                let node_id = self.arena.new_node(());
                self.node.insert(uuid, node_id);
                node_id
            }
        };

        self.name.insert(node_id, name);

        let parent_node_id = match self.node.get(&parent_uuid) {
            Some(p) => *p,
            None => {
                let parent_node_id = self.arena.new_node(());
                self.node.insert(parent_uuid, parent_node_id);
                parent_node_id
            }
        };
        parent_node_id.append(node_id, &mut self.arena);
    }
}

pub fn map() -> (Tree, HashMap<String, Uuid>) {
    let mut tree = Tree::new();
    let mut index = HashMap::new();

    for entry in fs::read_dir(DIR).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().map(|ext| ext.to_str()).flatten();
        match ext {
            Some(e) if e == "metadata" => (),
            Some(_) => continue,
            None => continue,
        }

        let uuid = Uuid(path.file_stem().unwrap().to_str().unwrap().to_owned());
        let metadata = fs::read_to_string(path).unwrap();
        let parent_uuid = Uuid(parent(&metadata).unwrap().to_owned());
        let name = name(&metadata).unwrap().to_owned();
        index.insert(name.clone(), uuid.clone());

        match is_folder(&metadata) {
            true => tree.add_folder(uuid, parent_uuid, name),
            false => tree.add_file(uuid, parent_uuid),
        }
    }
    (tree, index)
}

#[test]
fn extract_parent_id() {
    let metadata = r###"
{
    "deleted": false,
    "lastModified": "1633603894527",
    "lastOpened": "1572004477560",
    "lastOpenedPage": 1,
    "metadatamodified": false,
    "modified": false,
    "parent": "95318cc7-f844-416f-963a-cf277c83f10c",
    "pinned": false,
    "synced": true,
    "type": "DocumentType",
    "version": 1,
    "visibleName": "Paper selection"
}
"###;

    assert_eq!(
        Some("95318cc7-f844-416f-963a-cf277c83f10c"),
        parent(metadata)
    )
}

#[cfg(test)]
fn test_tree() -> Tree {
    let node_parent_pairs = [
        ("a0", ""),
        ("b1", "B0"),
        ("B1", "B0"),
        ("b0", ""),
        ("a2", "A1"),
        ("A1", "A0"),
        ("a1", "A0"),
        ("A0", ""),
        ("B0", ""),
    ];

    let mut tree = Tree::new();
    for (name, parent) in node_parent_pairs {
        if name.chars().next().unwrap().is_uppercase() {
            tree.add_folder(name.into(), Uuid(parent.to_owned()), name.into());
        } else {
            tree.add_file(name.into(), Uuid(parent.to_owned()));
        }
    }
    tree
}

#[test]
fn tree() {
    // folders in CAPS, files normal chars
    // ROOT
    // |-- a0
    // |-- b0
    // |-- A0
    // |   |-- a1
    // |   |-- A1
    // |       |-- a2
    // |-- B0
    // |   |-- b1
    // |   |-- B1

    let tree = test_tree();

    let root_node = tree.root(Uuid("".to_owned()));
    let root_name = tree.name.get(root_node).unwrap();
    assert_eq!("", root_name);

    let print = format!("{tree}");
    let correct = r###"    
    |-- a0
    |-- b0
    |-- A0
        |-- a1
        |-- A1
            |-- a2
    |-- B0
        |-- b1
        |-- B1
"###;
    assert_eq!(print, correct);
}

#[test]
fn children() {
    let tree = test_tree();
    let (files, folders) = tree.children("A0".into()).unwrap();
    assert_eq!(files, vec!("a1".into(), "a2".into()));
    assert_eq!(folders, vec!("A0".to_owned(), "A1".to_owned()));
}

#[test]
fn root_children() {
    let tree = test_tree();
    let (files, folders) = tree.children("".into()).unwrap();
    assert_eq!(
        files,
        vec!(
            "a0".into(),
            "b0".into(),
            "a1".into(),
            "a2".into(),
            "b1".into()
        )
    );
    assert_eq!(
        folders,
        vec!(
            "".to_owned(),
            "A0".to_owned(),
            "A1".to_owned(),
            "B0".to_owned(),
            "B1".to_owned()
        )
    );
}
