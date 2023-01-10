use std::collections::HashMap;
use std::fmt::Display;
use std::fs;
use std::path::{Path, PathBuf};

use color_eyre::{eyre::WrapErr, Result};
use indextree::{Arena, NodeId};
use regex::Regex;

#[cfg(target_arch = "arm")]
pub const DIR: &str = "/home/root/.local/share/remarkable/xochitl";
#[cfg(not(target_arch = "arm"))]
pub const DIR: &str = "data/xochitl";

fn extract_field<'a>(metadata: &'a str, field: &str) -> Option<&'a str> {
    let pattern = format!("\"{field}\": ?(?:\"(.*?)\"|.*?)(?:,|\n|}})");
    let re = Regex::new(&pattern).expect(&format!(
        "Unable to parse pattern {pattern} to Regex object"
    ));
    let value = re.captures(metadata)?.get(1)?.as_str();

    Some(value)
}

fn parent(metadata: &str) -> Option<&str> {
    extract_field(metadata, "parent")
}

fn name(metadata: &str) -> Option<&str> {
    extract_field(metadata, "visibleName")
}

fn is_folder(metadata: &str) -> bool {
    let doc_type = extract_field(metadata, "type").unwrap();
    match doc_type {
        "DocumentType" => false,
        "CollectionType" => true,
        _t => panic!("unexpected document type: {_t}"),
    }
}

#[derive(Eq, PartialEq, Hash, Debug, Clone)]
pub struct Uuid(String);

impl AsRef<Path> for Uuid {
    fn as_ref(&self) -> &Path {
        Path::new(&self.0)
    }
}

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

pub struct File {
    uuid: Uuid,
    name: String,
}

impl Display for File {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub struct Tree {
    arena: Arena<()>,
    node: HashMap<Uuid, NodeId>,
    name: HashMap<NodeId, String>,
    files: HashMap<NodeId, Vec<File>>,
}

impl Display for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let node_id = self.root(Uuid("".to_owned()));
        self.print_recurse(*node_id, 0, f)?;
        Ok(())
    }
}

pub struct SubTree<'a> {
    tree: &'a Tree,
    pub path: PathBuf,
    root: NodeId,
}

impl<'a> Display for SubTree<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.tree.print_recurse(self.root, 0, f)?;
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

    fn path(&self, node: &NodeId) -> PathBuf {
        node.ancestors(&self.arena)
            .map(|id| self.name.get(&id).unwrap())
            .map(Path::new)
            .collect()
    }

    pub fn subtree(&self, node: NodeId) -> SubTree {
        SubTree {
            tree: self,
            path: self.path(&node),
            root: node,
        }
    }

    pub fn print_recurse(
        &self,
        node: NodeId,
        indent: usize,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let ident_str: String = std::iter::once(' ').cycle().take(indent * 4).collect();
        let node_name = self.name.get(&node).unwrap();
        match indent {
            0 => writeln!(f, "{node_name}")?,
            _ => writeln!(f, "{ident_str}|-- {node_name}")?,
        }
        if let Some(files) = self.files.get(&node) {
            let mut names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
            names.sort_unstable();
            for name in names {
                writeln!(f, "{ident_str}    |-- {name}")?;
            }
        }

        for child in node.children(&self.arena) {
            self.print_recurse(child, indent + 1, f)?;
        }
        Ok(())
    }

    pub fn root(&self, uuid: Uuid) -> &NodeId {
        self.node.get(&uuid).unwrap()
    }

    pub fn node_for(&self, path: &str) -> std::result::Result<NodeId, String> {
        let mut node = *self.root(Uuid("".to_owned()));
        // find the right node
        if !path.is_empty() {
            for comp in path.split('/') {
                node = node
                    .children(&self.arena)
                    .find(|n| {
                        let name = self.name.get(n).unwrap();
                        name == comp
                    })
                    .ok_or_else(|| path.to_owned())?;
            }
        }
        Ok(node)
    }

    pub fn descendant_files(&self, subroot: NodeId) -> Result<Vec<Uuid>> {
        let mut files = Vec::new();
        for folder in subroot.descendants(&self.arena) {
            if let Some(content) = self.files.get(&folder) {
                files.extend(content.iter().map(|f| f.uuid.clone()));
            }
        }
        Ok(files)
    }

    pub fn add_file(&mut self, uuid: Uuid, parent_uuid: Uuid, name: String) {
        let parent_node = match self.node.get(&parent_uuid) {
            Some(n) => *n,
            None => {
                let parent_node = self.arena.new_node(());
                self.node.insert(parent_uuid, parent_node);
                parent_node
            }
        };
        let file = File { uuid, name };
        match self.files.get_mut(&parent_node) {
            Some(list) => list.push(file),
            None => {
                self.files.insert(parent_node, vec![file]);
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

pub fn map() -> Result<(Tree, HashMap<String, Uuid>)> {
    let mut tree = Tree::new();
    let mut index = HashMap::new();

    for entry in fs::read_dir(DIR).wrap_err("remarkable data directory not found")? {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|ext| ext.to_str());
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
            false => tree.add_file(uuid, parent_uuid, name),
        }
    }
    Ok((tree, index))
}

#[cfg(test)]
pub mod test {
    use super::*;

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

    #[test]
    fn extract_parent_id_with_spaces() {
        let metadata = r#"{"visibleName":"CMS","type":"CollectionType","parent":"0b7d1978-dc97-4433-8e31-ad6ff7fe1cf7","lastModified":"1654958754102943861","lastOpened":"","version":0,"pinned":false,"synced":true,"modified":false,"deleted":false,"metadatamodified":false}"#;
        assert_eq!(
            Some("0b7d1978-dc97-4433-8e31-ad6ff7fe1cf7"),
            parent(metadata)
        )
    }

    #[test]
    fn extract_visiblename_ending_with_lineend() {
        let metadata = r#"{
    "deleted": false,
    "lastModified": "1643992474183",
    "lastOpened": "1643992259259",
    "lastOpenedPage": 0,
    "metadatamodified": false,
    "modified": false,
    "parent": "3055805b-54c9-4950-9492-ff97ee603764",
    "pinned": false,
    "synced": true,
    "type": "DocumentType",
    "version": 2,
    "visibleName": "Book recs"
}
"#;
        assert_eq!(Some("Book recs"), name(metadata));
    }

    #[test]
    fn extract_type_with_spaces() {
        let metadata = "{\n    \"deleted\": false,\n    \"lastModified\": \"1643992474183\",\n    \"lastOpened\": \"1643992259259\",\n    \"lastOpenedPage\": 0,\n    \"metadatamodified\": false,\n    \"modified\": false,\n    \"parent\": \"3055805b-54c9-4950-9492-ff97ee603764\",\n    \"pinned\": false,\n    \"synced\": true,\n    \"type\": \"DocumentType\",\n    \"version\": 2,\n    \"visibleName\": \"Book recs\"\n}\n";
        assert!(!is_folder(metadata));
    }

    #[test]
    fn extract_visiblename_ending_with_bracket() {
        let metadata = r#"{"deleted":false,"lastModified":"1673176298000","lastOpened":"","lastOpenedPage":0,"metadatamodified":false,"modified":false,"parent":"816d93cc-1b07-442b-b16c-9a941a3f647c","pinned":false,"synced":false,"type":"CollectionType","version":0,"visibleName":"Missing semester"}"#;
        assert_eq!(Some("Missing semester"), name(metadata));
    }

    #[cfg(test)]
    pub fn test_tree() -> Tree {
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
                tree.add_file(name.into(), Uuid(parent.to_owned()), name.to_owned());
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
        let node = tree.node_for("A0").unwrap();
        let files = tree.descendant_files(node).unwrap();
        assert_eq!(files, vec!("a1".into(), "a2".into()));
    }

    #[test]
    fn root_children() {
        let tree = test_tree();
        let node = tree.node_for("").unwrap();
        let files = tree.descendant_files(node).unwrap();
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
    }
}
