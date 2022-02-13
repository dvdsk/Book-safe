use std::collections::HashMap;
use std::fmt::Display;
use std::fs;

use color_eyre::{Result, eyre::eyre};
use indextree::{Arena, NodeId};

const DIR: &str = "data/xochitl";

fn extract_field<'a>(metadata: &'a str, pattern: &str) -> Option<&'a str> {
    let a = metadata.find(pattern)? + pattern.len();
    let b = a + metadata[a..].find('\"').unwrap();

    Some(&metadata[a..b])
}

fn parent_id(metadata: &str) -> Option<&str> {
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

#[derive(Default)]
pub struct Tree {
    arena: Arena<()>,
    node: HashMap<Uuid, NodeId>,
    name: HashMap<NodeId, String>,
    files: HashMap<NodeId, Vec<Uuid>>,
}

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let to_name = self.to_name();

        let (_, node_id) = self.root();
        self.print(&to_name, *node_id, 0, f)?;
        Ok(())
    }
}

impl Tree {
    pub fn to_name(&self) -> HashMap<NodeId, Uuid> {
        let to_name: HashMap<NodeId, Uuid> = self
            .node
            .iter()
            .map(|(k, v)| (v.to_owned(), (*k).clone()))
            .collect();
        to_name
    }

    pub fn print(
        &self,
        to_name: &HashMap<NodeId, Uuid>,
        node: NodeId,
        indent: usize,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let node_name = to_name.get(&node).unwrap();
        let ident_str: String = std::iter::once(' ').cycle().take(indent * 4).collect();
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
            self.print(to_name, child, indent + 1, f)?;
        }
        Ok(())
    }

    pub fn root(&self) -> (&Uuid, &NodeId) {
        for (name, node_id) in &self.node {
            let node = self.arena.get(*node_id).unwrap();
            if node.parent().is_none() {
                return (name, node_id);
            }
        }
        panic!("tree should have a root")
    }

    pub fn children(
        &self,
        path: String,
    ) -> Result<(Vec<Uuid>, Vec<String>)> {
        let mut files = Vec::new();
        let mut folders = Vec::new();

        let mut node = *self.root().1;
        for comp in path.split('/') {
            node = node
                .children(&self.arena)
                .find(|n| {
                    let name = self.name.get(n).unwrap();
                    name == comp
                })
                .ok_or_else(|| eyre!("path incorrect, failded to find component: {comp}"))?;
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

    pub fn add_file(&mut self, id: Uuid, parent: Option<Uuid>) {
        let parent = parent.unwrap();
        let parent_node = match self.node.get(&parent) {
            Some(n) => *n,
            None => {
                let parent_node = self.arena.new_node(());
                self.node.insert(parent, parent_node);
                parent_node
            }
        };
        match self.files.get_mut(&parent_node) {
            Some(list) => list.push(id),
            None => {
                self.files.insert(parent_node, vec![id]);
            }
        }
    }

    pub fn add_folder(&mut self, id: Uuid, parent: Option<Uuid>, name: String) {
        let node_id = match self.node.get(&id) {
            Some(node) => *node,
            None => {
                let node_id = self.arena.new_node(());
                self.node.insert(id, node_id);
                node_id
            }
        };

        self.name.insert(node_id, name);

        if let Some(p) = parent {
            let parent_node_id = match self.node.get(&p) {
                Some(p) => *p,
                None => {
                    let parent_node_id = self.arena.new_node(());
                    self.node.insert(p, parent_node_id);
                    parent_node_id
                }
            };
            parent_node_id.append(node_id, &mut self.arena);
        }
    }
}

pub fn map() -> (Tree, HashMap<String, Uuid>) {
    let mut tree = Tree::default();
    let mut index = HashMap::new();

    for entry in fs::read_dir(DIR).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().map(|ext| ext.to_str()).flatten();
        match ext {
            Some(e) if e == "metadata" => (),
            Some(_) => continue,
            None => continue,
        }

        let id = Uuid(path.file_stem().unwrap().to_str().unwrap().to_owned());
        let metadata = fs::read_to_string(path).unwrap();
        let parent_id = parent_id(&metadata).map(str::to_owned).map(Uuid);
        let name = name(&metadata).unwrap().to_owned();
        index.insert(name.clone(), id.clone());

        match is_folder(&metadata) {
            true => tree.add_folder(id, parent_id, name),
            false => tree.add_file(id, parent_id),
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
        parent_id(metadata)
    )
}

#[cfg(test)]
fn test_tree() -> Tree {
    let node_parent_pairs = [
        ("a0", Some("ROOT")),
        ("b1", Some("B0")),
        ("B1", Some("B0")),
        ("b0", Some("ROOT")),
        ("ROOT", None),
        ("a2", Some("A1")),
        ("A1", Some("A0")),
        ("a1", Some("A0")),
        ("A0", Some("ROOT")),
        ("B0", Some("ROOT")),
    ];

    let mut tree = Tree::default();
    for (name, parent) in node_parent_pairs {
        if name.chars().next().unwrap().is_uppercase() {
            tree.add_folder(name.into(), parent.map(str::to_owned).map(Uuid), name.into());
        } else {
            tree.add_file(name.into(), parent.map(Uuid::from));
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

    assert_eq!(tree.root().0 .0, "ROOT");
    let print = format!("{tree}");
    let correct = r###"    ROOT
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
    let (files, folders) = tree.children(std::iter::once("A0")).unwrap();
    assert_eq!(files, vec!("a1".into(), "a2".into()));
    assert_eq!(folders, vec!("A0".to_owned(), "A1".to_owned()));
}
