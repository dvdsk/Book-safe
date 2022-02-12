use std::collections::HashMap;
use std::fmt::Display;
use std::fs;

use indextree::{Arena, NodeId};

const DIR: &str = "data/xochitl";

fn extract_field<'a>(metadata: &'a str, pattern: &str) -> Option<&'a str> {
    let a = metadata.find(pattern)? + pattern.len();
    let b = a + &metadata[a..].find("\"").unwrap();

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

type Entry = String;
#[derive(Default)]
pub struct Tree {
    arena: Arena<()>,
    to_node: HashMap<Entry, NodeId>,
    to_files: HashMap<Entry, Vec<Entry>>,
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
    pub fn to_name(&self) -> HashMap<NodeId, Entry> {
        let to_name: HashMap<NodeId, Entry> = self
            .to_node
            .iter()
            .map(|(k, v)| (v.to_owned(), k.to_owned()))
            .collect();
        to_name
    }

    pub fn print(
        &self,
        to_name: &HashMap<NodeId, Entry>,
        node: NodeId,
        indent: usize,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let node_name = to_name.get(&node).unwrap();
        let ident_str: String = std::iter::once(' ').cycle().take(indent * 4).collect();
        match indent {
            0 => write!(f, "    {node_name}\n")?,
            _ => write!(f, "{ident_str}|-- {node_name}\n")?,
        }
        if let Some(files) = self.to_files.get(node_name) {
            for file in files {
                write!(f, "{ident_str}    |-- {file}\n")?;
            }
        }

        for child in node.children(&self.arena) {
            self.print(to_name, child, indent + 1, f)?;
        }
        Ok(())
    }

    pub fn root(&self) -> (&str, &NodeId) {
        for (name, node_id) in &self.to_node {
            let node = self.arena.get(*node_id).unwrap();
            if node.parent().is_none() {
                return (name, node_id);
            }
        }
        panic!("tree should have a root")
    }

    pub fn children(&self, dir: &str) -> Option<(Vec<Entry>, Vec<Entry>)> {
        let mut files = Vec::new();
        let mut folders = Vec::new();

        let to_name = self.to_name();
        let node = self.to_node.get(dir)?;

        for folder in node
            .descendants(&self.arena)
            .map(|id| to_name.get(&id).unwrap())
        {
            if let Some(content) = self.to_files.get(folder) {
                files.extend_from_slice(content);
            }
            folders.push(folder.to_owned());
        }
        Some((files, folders))
    }

    pub fn add_file(&mut self, id: String, parent: Option<&str>) {
        let parent = parent.unwrap();
        match self.to_files.get_mut(parent) {
            Some(list) => list.push(id),
            None => {
                self.to_files.insert(parent.to_owned(), vec![id]);
            }
        }
    }

    pub fn add_folder(&mut self, id: String, parent: Option<String>) {
        let node_id = match self.to_node.get(&id) {
            Some(node) => *node,
            None => {
                let node_id = self.arena.new_node(());
                self.to_node.insert(id, node_id.clone());
                node_id
            }
        };

        if let Some(p) = parent {
            let parent_node_id = match self.to_node.get(&p) {
                Some(p) => *p,
                None => {
                    let parent_node_id = self.arena.new_node(());
                    self.to_node.insert(p, parent_node_id.clone());
                    parent_node_id
                }
            };
            parent_node_id.append(node_id, &mut self.arena);
        }
    }
}

pub fn map() -> (Tree, HashMap<String, String>) {
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

        let id = path.file_name().unwrap().to_str().unwrap().to_owned();
        let metadata = fs::read_to_string(path).unwrap();
        let parent_id = parent_id(&metadata);
        let name = name(&metadata).unwrap().to_owned();
        index.insert(name, id.clone());

        match is_folder(&metadata) {
            true => tree.add_folder(id, parent_id.map(str::to_owned)),
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
            tree.add_folder(name.into(), parent.map(str::to_owned));
        } else {
            tree.add_file(name.into(), parent);
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

    assert_eq!(tree.root().0, "ROOT");
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
    let (files, folders) = tree.children("A0").unwrap();
    assert_eq!(files, vec!("a1", "a2"));
    assert_eq!(folders, vec!("A0", "A1"));
}
