use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

// ══════════════════════════════════════════════════════════════════
// Core types
// ══════════════════════════════════════════════════════════════════

pub type NodeId = u64;

#[derive(Clone, Debug)]
pub enum Node {
    File { content: Vec<u8> },
    Dir { entries: BTreeMap<String, NodeId> },
}

impl Node {
    pub fn is_dir(&self) -> bool {
        matches!(self, Node::Dir { .. })
    }
}

#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
}

/// Resource limits for the in-memory filesystem.
pub struct FsLimits {
    /// Maximum size of a single file in bytes.
    pub max_file_size: usize,
    /// Maximum number of entries in a single directory.
    pub max_dir_entries: usize,
    /// Maximum total nodes (files + dirs) in MemoryFs.
    pub max_total_nodes: usize,
}

impl Default for FsLimits {
    fn default() -> Self {
        FsLimits {
            max_file_size: 100 * 1024 * 1024, // 100 MB
            max_dir_entries: 100_000,
            max_total_nodes: 1_000_000,
        }
    }
}

/// Filesystem operating mode, specified at creation time.
pub enum FsMode {
    /// All reads and writes in memory. Nothing touches disk.
    Memory,
    /// Reads fall through to disk; all writes stay in-memory.
    ReadThrough(PathBuf),
    /// All reads and writes go to disk.
    Passthrough(PathBuf),
}

// ══════════════════════════════════════════════════════════════════
// FileSystem trait
// ══════════════════════════════════════════════════════════════════

pub(crate) trait FileSystem {
    fn resolve(&self, path: &str, cwd: &str) -> Option<()>;
    fn mkdir(&mut self, path: &str, cwd: &str) -> bool;
    fn create_file(&mut self, path: &str, cwd: &str) -> bool;
    fn read_file(&self, path: &str, cwd: &str) -> Option<Vec<u8>>;
    fn write_file(&mut self, path: &str, cwd: &str, data: &[u8]) -> bool;
    fn list_dir(&self, path: &str, cwd: &str, show_hidden: bool) -> Option<Vec<DirEntry>>;
    fn remove(&mut self, path: &str, cwd: &str) -> bool;
    fn remove_all(&mut self, path: &str, cwd: &str) -> bool;
    fn copy_file(&mut self, src: &str, dst: &str, cwd: &str) -> bool;
    fn move_node(&mut self, src: &str, dst: &str, cwd: &str) -> bool;
    fn exists(&self, path: &str, cwd: &str) -> bool;
    fn is_dir(&self, path: &str, cwd: &str) -> bool;
    fn find(&self, start_path: &str, pattern: &str) -> Vec<String>;
}

// ══════════════════════════════════════════════════════════════════
// Fs – public wrapper
// ══════════════════════════════════════════════════════════════════

pub struct Fs {
    inner: Box<dyn FileSystem>,
}

impl Fs {
    pub fn new() -> Self {
        Fs {
            inner: Box::new(MemoryFs::new()),
        }
    }

    pub fn with_mode(mode: FsMode) -> Self {
        Fs {
            inner: match mode {
                FsMode::Memory => Box::new(MemoryFs::new()),
                FsMode::ReadThrough(root) => Box::new(ReadThroughFs::new(root)),
                FsMode::Passthrough(root) => Box::new(PassthroughFs::new(root)),
            },
        }
    }

    /// Create a Memory-mode shell with custom resource limits.
    pub fn with_limits(limits: FsLimits) -> Self {
        Fs {
            inner: Box::new(MemoryFs::with_limits(limits)),
        }
    }

    // ── Path helpers (shared) ─────────────────────────────────

    pub fn normalize(&self, path: &str) -> String {
        normalize_path(path)
    }

    pub fn resolve(&self, path: &str, cwd: &str) -> Option<()> {
        self.inner.resolve(path, cwd)
    }

    pub fn resolve_abs(&self, path: &str, cwd: &str) -> String {
        resolve_abs(path, cwd)
    }

    pub fn split_path<'a>(&self, path: &'a str) -> (&'a str, &'a str) {
        split_path(path)
    }

    // ── Delegate all operations ───────────────────────────────

    pub fn mkdir(&mut self, path: &str, cwd: &str) -> bool {
        self.inner.mkdir(path, cwd)
    }

    pub fn create_file(&mut self, path: &str, cwd: &str) -> bool {
        self.inner.create_file(path, cwd)
    }

    pub fn read_file(&self, path: &str, cwd: &str) -> Option<Vec<u8>> {
        self.inner.read_file(path, cwd)
    }

    pub fn write_file(&mut self, path: &str, cwd: &str, data: &[u8]) -> bool {
        self.inner.write_file(path, cwd, data)
    }

    pub fn list_dir(&self, path: &str, cwd: &str, show_hidden: bool) -> Option<Vec<DirEntry>> {
        self.inner.list_dir(path, cwd, show_hidden)
    }

    pub fn remove(&mut self, path: &str, cwd: &str) -> bool {
        self.inner.remove(path, cwd)
    }

    pub fn remove_all(&mut self, path: &str, cwd: &str) -> bool {
        self.inner.remove_all(path, cwd)
    }

    pub fn copy_file(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        self.inner.copy_file(src, dst, cwd)
    }

    pub fn move_node(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        self.inner.move_node(src, dst, cwd)
    }

    pub fn exists(&self, path: &str, cwd: &str) -> bool {
        self.inner.exists(path, cwd)
    }

    pub fn is_dir(&self, path: &str, cwd: &str) -> bool {
        self.inner.is_dir(path, cwd)
    }

    pub fn find(&self, start_path: &str, pattern: &str) -> Vec<String> {
        self.inner.find(start_path, pattern)
    }
}

impl Default for Fs {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════
// Shared path helpers
// ══════════════════════════════════════════════════════════════════

fn normalize_path(path: &str) -> String {
    let path = path.trim();
    let (is_abs, rest) = if path.starts_with('/') {
        (true, &path[1..])
    } else {
        (false, path)
    };

    let mut components: Vec<&str> = Vec::new();
    for part in rest.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            _ => components.push(part),
        }
    }

    if is_abs {
        "/".to_string() + &components.join("/")
    } else if components.is_empty() {
        ".".to_string()
    } else {
        components.join("/")
    }
}

fn resolve_abs(path: &str, cwd: &str) -> String {
    if path.starts_with('/') {
        normalize_path(path)
    } else {
        let base = if cwd == "/" {
            String::new()
        } else {
            cwd.to_string()
        };
        normalize_path(&format!("{}/{}", base, path))
    }
}

fn split_path(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(idx) => {
            let parent = if idx == 0 { "/" } else { &path[..idx] };
            let name = &path[idx + 1..];
            (parent, name)
        }
        None => (".", path),
    }
}

/// Map a virtual shell path (`/foo/bar`) to a real filesystem path.
fn to_real_path(disk_root: &Path, virtual_path: &str) -> PathBuf {
    let relative = virtual_path.strip_prefix('/').unwrap_or(virtual_path);
    if relative.is_empty() {
        disk_root.to_path_buf()
    } else {
        disk_root.join(relative)
    }
}

/// Extract the filename component of a path.
fn fs_name(path: &str) -> &str {
    match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

// ══════════════════════════════════════════════════════════════════
// MemoryFs – pure in-memory (original behaviour)
// ══════════════════════════════════════════════════════════════════

pub(crate) struct MemoryFs {
    nodes: HashMap<NodeId, Node>,
    next_id: NodeId,
    root: NodeId,
    limits: FsLimits,
}

impl MemoryFs {
    pub fn new() -> Self {
        Self::with_limits(FsLimits::default())
    }

    pub fn with_limits(limits: FsLimits) -> Self {
        let mut nodes = HashMap::new();
        nodes.insert(
            0,
            Node::Dir {
                entries: BTreeMap::new(),
            },
        );
        MemoryFs {
            nodes,
            next_id: 1,
            root: 0,
            limits,
        }
    }

    fn alloc_id(&mut self) -> Option<NodeId> {
        if self.nodes.len() >= self.limits.max_total_nodes {
            return None;
        }
        let id = self.next_id;
        self.next_id += 1;
        Some(id)
    }

    fn walk(&self, start: NodeId, components: &[&str]) -> Option<NodeId> {
        let mut current = start;
        for &comp in components {
            match self.nodes.get(&current)? {
                Node::Dir { entries } => {
                    current = *entries.get(comp)?;
                }
                Node::File { .. } => return None,
            }
        }
        Some(current)
    }

    fn remove_subtree(&mut self, id: NodeId) {
        let mut stack = vec![id];
        while let Some(current_id) = stack.pop() {
            if let Some(Node::Dir { entries }) = self.nodes.remove(&current_id) {
                for (_, child_id) in entries {
                    stack.push(child_id);
                }
            }
        }
    }

    fn resolve_node_id(&self, path: &str, cwd: &str) -> Option<NodeId> {
        let abs = resolve_abs(path, cwd);
        if abs.is_empty() || abs == "/" {
            return Some(self.root);
        }
        let components: Vec<&str> = abs[1..].split('/').collect();
        self.walk(self.root, &components)
    }
}

impl FileSystem for MemoryFs {
    fn resolve(&self, path: &str, cwd: &str) -> Option<()> {
        self.resolve_node_id(path, cwd).map(|_| ())
    }

    fn mkdir(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        let (parent_path, name) = split_path(&abs);
        let parent_id = match self.resolve_node_id(parent_path, "/") {
            Some(id) => id,
            None => return false,
        };
        let id = match self.alloc_id() {
            Some(id) => id,
            None => return false,
        };
        match self.nodes.get_mut(&parent_id) {
            Some(Node::Dir { entries }) => {
                if entries.contains_key(name) {
                    return false;
                }
                if entries.len() >= self.limits.max_dir_entries {
                    return false;
                }
                entries.insert(name.to_string(), id);
                self.nodes.insert(
                    id,
                    Node::Dir {
                        entries: BTreeMap::new(),
                    },
                );
                true
            }
            _ => false,
        }
    }

    fn create_file(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        let (parent_path, name) = split_path(&abs);
        let parent_id = match self.resolve_node_id(parent_path, "/") {
            Some(id) => id,
            None => return false,
        };
        let id = match self.alloc_id() {
            Some(id) => id,
            None => return false,
        };
        match self.nodes.get_mut(&parent_id) {
            Some(Node::Dir { entries }) => {
                if entries.contains_key(name) {
                    return false;
                }
                if entries.len() >= self.limits.max_dir_entries {
                    return false;
                }
                entries.insert(name.to_string(), id);
                self.nodes.insert(id, Node::File { content: vec![] });
                true
            }
            _ => false,
        }
    }

    fn read_file(&self, path: &str, cwd: &str) -> Option<Vec<u8>> {
        let node_id = self.resolve_node_id(path, cwd)?;
        match self.nodes.get(&node_id)? {
            Node::File { content } => Some(content.clone()),
            Node::Dir { .. } => None,
        }
    }

    fn write_file(&mut self, path: &str, cwd: &str, data: &[u8]) -> bool {
        if data.len() > self.limits.max_file_size {
            return false;
        }

        let abs = resolve_abs(path, cwd);

        if let Some(node_id) = self.resolve_node_id(&abs, "/") {
            match self.nodes.get_mut(&node_id) {
                Some(Node::File { content }) => {
                    *content = data.to_vec();
                    return true;
                }
                Some(Node::Dir { .. }) => return false,
                None => return false,
            }
        }

        let (parent_path, name) = split_path(&abs);
        let parent_id = match self.resolve_node_id(parent_path, "/") {
            Some(id) => id,
            None => return false,
        };
        let id = match self.alloc_id() {
            Some(id) => id,
            None => return false,
        };
        match self.nodes.get_mut(&parent_id) {
            Some(Node::Dir { entries }) => {
                if entries.len() >= self.limits.max_dir_entries {
                    return false;
                }
                entries.insert(name.to_string(), id);
                self.nodes.insert(
                    id,
                    Node::File {
                        content: data.to_vec(),
                    },
                );
                true
            }
            _ => false,
        }
    }

    fn list_dir(&self, path: &str, cwd: &str, show_hidden: bool) -> Option<Vec<DirEntry>> {
        let node_id = self.resolve_node_id(path, cwd)?;
        match self.nodes.get(&node_id)? {
            Node::Dir { entries } => {
                let result = entries
                    .iter()
                    .filter(|(name, _)| show_hidden || !name.starts_with('.'))
                    .map(|(name, &child_id)| {
                        let child = self.nodes.get(&child_id).unwrap();
                        DirEntry {
                            name: name.clone(),
                            is_dir: child.is_dir(),
                            size: match child {
                                Node::File { content } => content.len() as u64,
                                Node::Dir { .. } => 0,
                            },
                        }
                    })
                    .collect();
                Some(result)
            }
            Node::File { .. } => None,
        }
    }

    fn remove(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        let (parent_path, name) = split_path(&abs);
        let parent_id = match self.resolve_node_id(parent_path, "/") {
            Some(id) => id,
            None => return false,
        };
        let child_id = match self.nodes.get(&parent_id) {
            Some(Node::Dir { entries }) => entries.get(name).copied(),
            _ => None,
        };
        match child_id {
            Some(cid) => {
                if self.nodes.get(&cid).map_or(false, |n| n.is_dir()) {
                    return false;
                }
                if let Some(Node::Dir { entries }) = self.nodes.get_mut(&parent_id) {
                    entries.remove(name);
                }
                self.nodes.remove(&cid);
                true
            }
            None => false,
        }
    }

    fn remove_all(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        if abs == "/" {
            return false;
        }
        let (parent_path, name) = split_path(&abs);
        let parent_id = match self.resolve_node_id(parent_path, "/") {
            Some(id) => id,
            None => return false,
        };
        let child_id = match self.nodes.get(&parent_id) {
            Some(Node::Dir { entries }) => entries.get(name).copied(),
            _ => None,
        };
        match child_id {
            Some(cid) => {
                if let Some(Node::Dir { entries }) = self.nodes.get_mut(&parent_id) {
                    entries.remove(name);
                }
                self.remove_subtree(cid);
                true
            }
            None => false,
        }
    }

    fn copy_file(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        let src_abs = resolve_abs(src, cwd);
        let src_id = match self.resolve_node_id(&src_abs, "/") {
            Some(id) => id,
            None => return false,
        };
        let content = match self.nodes.get(&src_id) {
            Some(Node::File { content }) => content.clone(),
            _ => return false,
        };

        let dst_abs = resolve_abs(dst, cwd);
        if let Some(dst_id) = self.resolve_node_id(&dst_abs, "/") {
            if let Some(Node::Dir { .. }) = self.nodes.get(&dst_id) {
                let name = fs_name(&src_abs);
                let child_path = if dst_abs == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", dst_abs, name)
                };
                return self.write_file(&child_path, "/", &content);
            }
        }
        self.write_file(&dst_abs, "/", &content)
    }

    fn move_node(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        let src_abs = resolve_abs(src, cwd);
        if src_abs == "/" {
            return false;
        }
        let (src_parent_path, src_name) = split_path(&src_abs);
        let src_parent_id = match self.resolve_node_id(src_parent_path, "/") {
            Some(id) => id,
            None => return false,
        };

        let dst_abs = resolve_abs(dst, cwd);

        let node_id = match self.nodes.get_mut(&src_parent_id) {
            Some(Node::Dir { entries }) => match entries.remove(src_name) {
                Some(id) => id,
                None => return false,
            },
            _ => return false,
        };

        let final_dst = if let Some(dst_id) = self.resolve_node_id(&dst_abs, "/") {
            if let Some(Node::Dir { .. }) = self.nodes.get(&dst_id) {
                if dst_abs == "/" {
                    format!("/{}", src_name)
                } else {
                    format!("{}/{}", dst_abs, src_name)
                }
            } else {
                dst_abs.clone()
            }
        } else {
            dst_abs.clone()
        };

        let (dst_parent_path, dst_name) = split_path(&final_dst);
        let dst_parent_id = match self.resolve_node_id(dst_parent_path, "/") {
            Some(id) => id,
            None => return false,
        };

        let old_id = match self.nodes.get(&dst_parent_id) {
            Some(Node::Dir { entries }) => entries.get(dst_name).copied(),
            _ => None,
        };
        if let Some(oid) = old_id {
            self.remove_subtree(oid);
        }
        match self.nodes.get_mut(&dst_parent_id) {
            Some(Node::Dir { entries }) => {
                entries.insert(dst_name.to_string(), node_id);
                true
            }
            _ => false,
        }
    }

    fn exists(&self, path: &str, cwd: &str) -> bool {
        self.resolve_node_id(path, cwd).is_some()
    }

    fn is_dir(&self, path: &str, cwd: &str) -> bool {
        match self.resolve_node_id(path, cwd) {
            Some(id) => self.nodes.get(&id).map_or(false, |n| n.is_dir()),
            None => false,
        }
    }

    fn find(&self, start_path: &str, pattern: &str) -> Vec<String> {
        let start_path = if start_path.is_empty() {
            "/"
        } else {
            start_path
        };
        let abs = normalize_path(start_path);
        let abs = if abs.is_empty() { "/".to_string() } else { abs };
        let mut results = Vec::new();
        if let Some(node_id) = self.resolve_node_id(&abs, "/") {
            self.find_recursive(node_id, &abs, pattern, &mut results, 0);
        }
        results
    }
}

impl MemoryFs {
    fn find_recursive(
        &self,
        node_id: NodeId,
        path: &str,
        pattern: &str,
        results: &mut Vec<String>,
        depth: usize,
    ) {
        if depth > 100 {
            return;
        }
        let filename = if path == "/" { "/" } else { fs_name(path) };
        if glob_matches(pattern, filename) {
            results.push(path.to_string());
        }
        if let Some(Node::Dir { entries }) = self.nodes.get(&node_id) {
            let children: Vec<(String, NodeId)> = entries
                .iter()
                .map(|(name, &id)| (name.clone(), id))
                .collect();
            for (name, child_id) in children {
                let child_path = if path == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", path, name)
                };
                self.find_recursive(child_id, &child_path, pattern, results, depth + 1);
            }
        }
    }
}

impl Default for MemoryFs {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════
// ReadThroughFs – reads from disk, writes stay in-memory
// ══════════════════════════════════════════════════════════════════
//
// Three maps track the overlay state:
//   files:       Path → file content
//   dir_entries: Path → BTreeMap<child_name, ()>  (directory contents)
//   deleted:     HashSet<Path>                     (hidden from all ops)

pub(crate) struct ReadThroughFs {
    disk_root: PathBuf,
    canonical_root: PathBuf,
    files: HashMap<String, Vec<u8>>,
    dir_entries: HashMap<String, BTreeMap<String, ()>>,
    deleted: std::collections::HashSet<String>,
}

impl ReadThroughFs {
    pub fn new(disk_root: PathBuf) -> Self {
        let canonical_root = disk_root
            .canonicalize()
            .unwrap_or_else(|_| disk_root.clone());
        let mut dir_entries = HashMap::new();
        dir_entries.insert("/".to_string(), BTreeMap::new());
        ReadThroughFs {
            disk_root,
            canonical_root,
            files: HashMap::new(),
            dir_entries,
            deleted: std::collections::HashSet::new(),
        }
    }

    fn real(&self, virtual_path: &str) -> PathBuf {
        to_real_path(&self.disk_root, virtual_path)
    }

    /// Resolve a virtual path to a real path, ensuring it stays within disk_root.
    fn sanitized_real(&self, virtual_path: &str) -> Option<PathBuf> {
        let real = self.real(virtual_path);
        let canonical = real.canonicalize().ok()?;
        if canonical.starts_with(&self.canonical_root) {
            Some(canonical)
        } else {
            None
        }
    }

    /// Populate overlay `dir_entries` for `path` from disk if not present.
    fn populate_dir(&mut self, path: &str) {
        if self.dir_entries.contains_key(path) || self.deleted.contains(path) {
            return;
        }
        let real = match self.sanitized_real(path) {
            Some(p) => p,
            None => return,
        };
        if let Ok(rd) = std::fs::read_dir(&real) {
            let mut entries = BTreeMap::new();
            for e in rd.flatten() {
                if let Ok(name) = e.file_name().into_string() {
                    entries.insert(name, ());
                }
            }
            self.dir_entries.insert(path.to_string(), entries);
        }
    }

    /// Ensure every directory on the way to `path` has its entries
    /// in `dir_entries`, populating from disk as needed.
    fn ensure_path(&mut self, path: &str) {
        if path == "/" || path.is_empty() {
            return;
        }
        let mut cur = String::new();
        for comp in path[1..].split('/') {
            let parent = cur.clone();
            if !cur.is_empty() {
                cur.push('/');
            }
            cur.push_str(comp);

            // Make sure parent has entries populated from disk
            self.populate_dir(&parent);

            // If parent exists but doesn't list this child, the child
            // might only exist on disk — re-populate to pick it up.
            if let Some(entries) = self.dir_entries.get(&parent) {
                if !entries.contains_key(comp) {
                    self.populate_dir(&parent);
                }
            }
        }
    }

    /// Walk the overlay directory tree, returning `Some(())` iff every
    /// component exists in `dir_entries` (or on disk as a fallback for
    /// the resolve operation).
    fn walk_overlay(&self, path: &str) -> bool {
        if self.deleted.contains(path) {
            return false;
        }
        if path == "/" {
            return self.dir_entries.contains_key("/");
        }
        let mut cur = String::new();
        for comp in path[1..].split('/') {
            let parent = cur.clone();
            if !cur.is_empty() {
                cur.push('/');
            }
            cur.push_str(comp);

            if self.deleted.contains(&cur) {
                return false;
            }
            match self.dir_entries.get(&parent) {
                Some(entries) => {
                    if !entries.contains_key(comp) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }

    /// Is `path` a directory in the overlay?
    fn is_overlay_dir(&self, path: &str) -> bool {
        self.dir_entries.contains_key(path) && !self.deleted.contains(path)
    }
}

// ── ancestors helper ───────────────────────────────────────────

/// Yield `/`, `/a`, `/a/b`, … for `/a/b/c`.
fn ancestors(path: &str) -> Vec<String> {
    let mut out = vec!["/".to_string()];
    if path == "/" {
        return out;
    }
    let mut cur = String::new();
    for comp in path[1..].split('/') {
        cur.push('/');
        cur.push_str(comp);
        out.push(cur.clone());
    }
    out
}

// ── FileSystem impl ────────────────────────────────────────────

impl FileSystem for ReadThroughFs {
    fn resolve(&self, path: &str, cwd: &str) -> Option<()> {
        let abs = resolve_abs(path, cwd);
        for a in ancestors(&abs) {
            if self.deleted.contains(&a) {
                return None;
            }
        }
        if self.walk_overlay(&abs) {
            return Some(());
        }
        // Fall through to disk
        self.sanitized_real(&abs).map(|_| ())
    }

    fn mkdir(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        if abs == "/" {
            return false;
        }
        let (parent_path, name) = split_path(&abs);

        // Make sure parent dir entries are in the overlay
        self.ensure_path(parent_path);

        // Parent must be a directory
        if !self.is_overlay_dir(parent_path) {
            let real = match self.sanitized_real(parent_path) {
                Some(p) => p,
                None => return false,
            };
            if !real.is_dir() {
                return false;
            }
            self.populate_dir(parent_path);
        }

        // Already exists (overlay)?
        if let Some(entries) = self.dir_entries.get(parent_path) {
            if entries.contains_key(name) {
                return false;
            }
        }
        // Already exists (disk)?
        if self.sanitized_real(&abs).is_some() {
            return false;
        }

        // Create in overlay
        self.dir_entries.insert(abs.clone(), BTreeMap::new());
        if let Some(entries) = self.dir_entries.get_mut(parent_path) {
            entries.insert(name.to_string(), ());
        }
        true
    }

    fn create_file(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        if abs == "/" {
            return false;
        }
        let (parent_path, name) = split_path(&abs);
        self.ensure_path(parent_path);

        // Parent must be a dir; name must not already exist
        if self.is_overlay_dir(parent_path) {
            if let Some(entries) = self.dir_entries.get(parent_path) {
                if entries.contains_key(name) {
                    return false;
                }
            }
        } else {
            let real = match self.sanitized_real(parent_path) {
                Some(p) => p,
                None => return false,
            };
            if !real.is_dir() {
                return false;
            }
            self.populate_dir(parent_path);
            if let Some(entries) = self.dir_entries.get(parent_path) {
                if entries.contains_key(name) {
                    return false;
                }
            }
        }

        self.files.insert(abs.clone(), Vec::new());
        if let Some(entries) = self.dir_entries.get_mut(parent_path) {
            entries.insert(name.to_string(), ());
        }
        true
    }

    fn read_file(&self, path: &str, cwd: &str) -> Option<Vec<u8>> {
        let abs = resolve_abs(path, cwd);
        for a in ancestors(&abs) {
            if self.deleted.contains(&a) {
                return None;
            }
        }
        if self.deleted.contains(&abs) {
            return None;
        }
        if let Some(content) = self.files.get(&abs) {
            return Some(content.clone());
        }
        // Fall through to disk
        if self.dir_entries.contains_key(&abs) {
            return None; // it's a directory
        }
        let real = self.sanitized_real(&abs)?;
        std::fs::read(real).ok()
    }

    fn write_file(&mut self, path: &str, cwd: &str, data: &[u8]) -> bool {
        let abs = resolve_abs(path, cwd);

        // Existing overlay file?
        if self.files.contains_key(&abs) {
            self.files.insert(abs, data.to_vec());
            return true;
        }
        // Overlay dir at this path?
        if self.is_overlay_dir(&abs) {
            return false;
        }

        // New file — parent must exist
        let (parent_path, name) = split_path(&abs);
        self.ensure_path(parent_path);

        let parent_ok = if self.is_overlay_dir(parent_path) {
            true
        } else {
            self.sanitized_real(parent_path)
                .map_or(false, |p| p.is_dir())
        };
        if !parent_ok {
            return false;
        }

        self.files.insert(abs.clone(), data.to_vec());
        self.populate_dir(parent_path);
        if let Some(entries) = self.dir_entries.get_mut(parent_path) {
            entries.insert(name.to_string(), ());
        }
        true
    }

    fn list_dir(&self, path: &str, cwd: &str, show_hidden: bool) -> Option<Vec<DirEntry>> {
        let abs = resolve_abs(path, cwd);
        for a in ancestors(&abs) {
            if self.deleted.contains(&a) {
                return None;
            }
        }
        if self.deleted.contains(&abs) {
            return None;
        }

        // If the path is known as a file in the overlay, it's not a dir
        if self.files.contains_key(&abs) {
            return None;
        }

        let mut merged: BTreeMap<String, bool> = BTreeMap::new();

        // Overlay entries
        if let Some(entries) = self.dir_entries.get(&abs) {
            for (name, _) in entries {
                let child = if abs == "/" {
                    format!("/{}", name)
                } else {
                    format!("{}/{}", abs, name)
                };
                if self.deleted.contains(&child) {
                    continue;
                }
                let is_dir = self.dir_entries.contains_key(&child);
                merged.insert(name.clone(), is_dir);
            }
        }

        // Disk entries
        let real = match self.sanitized_real(&abs) {
            Some(p) => p,
            None => return None,
        };
        if let Ok(rd) = std::fs::read_dir(&real) {
            for entry in rd.flatten() {
                if let Ok(name) = entry.file_name().into_string() {
                    if !show_hidden && name.starts_with('.') {
                        continue;
                    }
                    let child = if abs == "/" {
                        format!("/{}", name)
                    } else {
                        format!("{}/{}", abs, name)
                    };
                    if self.deleted.contains(&child) {
                        continue;
                    }
                    if !merged.contains_key(&name) {
                        let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
                        merged.insert(name, is_dir);
                    }
                }
            }
        }

        if merged.is_empty() && !self.dir_entries.contains_key(&abs) {
            // No overlay dir and nothing on disk
            if !std::fs::metadata(&real).map_or(false, |m| m.is_dir()) {
                return None;
            }
        }

        Some(
            merged
                .into_iter()
                .map(|(name, is_dir)| DirEntry {
                    name,
                    is_dir,
                    size: 0,
                })
                .collect(),
        )
    }

    fn remove(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        if abs == "/" {
            return false;
        }

        // Overlay file?
        if self.files.remove(&abs).is_some() {
            self.deleted.insert(abs.clone());
            let (parent, name) = split_path(&abs);
            if let Some(entries) = self.dir_entries.get_mut(parent) {
                entries.remove(name);
            }
            return true;
        }
        // Overlay dir? → refuse (use remove_all)
        if self.is_overlay_dir(&abs) {
            return false;
        }
        // Disk file?
        if let Some(real) = self.sanitized_real(&abs) {
            if real.is_file() {
                self.deleted.insert(abs);
                return true;
            }
        }
        false
    }

    fn remove_all(&mut self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        if abs == "/" {
            return false;
        }

        // Remove overlay nodes under this path
        let file_keys: Vec<String> = self
            .files
            .keys()
            .filter(|k| {
                k.starts_with(&abs) && (k.len() == abs.len() || k.as_bytes()[abs.len()] == b'/')
            })
            .cloned()
            .collect();
        for k in &file_keys {
            self.files.remove(k);
        }

        let dir_keys: Vec<String> = self
            .dir_entries
            .keys()
            .filter(|k| {
                k.starts_with(&abs) && (k.len() == abs.len() || k.as_bytes()[abs.len()] == b'/')
            })
            .cloned()
            .collect();
        for k in &dir_keys {
            self.dir_entries.remove(k);
        }

        // Mark disk children as deleted
        if let Some(real) = self.sanitized_real(&abs) {
            if let Ok(rd) = std::fs::read_dir(&real) {
                for entry in rd.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        let child = if abs == "/" {
                            format!("/{}", name)
                        } else {
                            format!("{}/{}", abs, name)
                        };
                        self.deleted.insert(child);
                    }
                }
            }
        }

        // Remove from parent entries
        let (parent, name) = split_path(&abs);
        if let Some(entries) = self.dir_entries.get_mut(parent) {
            entries.remove(name);
        }

        self.deleted.insert(abs);
        true
    }

    fn copy_file(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        let content = match self.read_file(src, cwd) {
            Some(c) => c,
            None => return false,
        };
        let src_abs = resolve_abs(src, cwd);
        let dst_abs = resolve_abs(dst, cwd);
        let final_dst = if self.is_dir(&dst_abs, "/") {
            let name = fs_name(&src_abs);
            if dst_abs == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", dst_abs, name)
            }
        } else {
            dst_abs
        };
        self.write_file(&final_dst, "/", &content)
    }

    fn move_node(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        let content = match self.read_file(src, cwd) {
            Some(c) => c,
            None => return false,
        };
        let src_abs = resolve_abs(src, cwd);
        if !self.remove(src, cwd) {
            return false;
        }
        let dst_abs = resolve_abs(dst, cwd);
        let final_dst = if self.is_dir(&dst_abs, "/") {
            let name = fs_name(&src_abs);
            if dst_abs == "/" {
                format!("/{}", name)
            } else {
                format!("{}/{}", dst_abs, name)
            }
        } else {
            dst_abs
        };
        self.write_file(&final_dst, "/", &content)
    }

    fn exists(&self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        for a in ancestors(&abs) {
            if self.deleted.contains(&a) {
                return false;
            }
        }
        if self.deleted.contains(&abs) {
            return false;
        }
        if self.files.contains_key(&abs) || self.dir_entries.contains_key(&abs) {
            return true;
        }
        self.sanitized_real(&abs).is_some()
    }

    fn is_dir(&self, path: &str, cwd: &str) -> bool {
        let abs = resolve_abs(path, cwd);
        for a in ancestors(&abs) {
            if self.deleted.contains(&a) {
                return false;
            }
        }
        if self.deleted.contains(&abs) {
            return false;
        }
        if self.dir_entries.contains_key(&abs) {
            return true;
        }
        if self.files.contains_key(&abs) {
            return false;
        }
        self.sanitized_real(&abs).map_or(false, |p| p.is_dir())
    }

    fn find(&self, start_path: &str, pattern: &str) -> Vec<String> {
        find_default(self, start_path, pattern)
    }
}

// ══════════════════════════════════════════════════════════════════
// PassthroughFs – all operations go to the real filesystem
// ══════════════════════════════════════════════════════════════════

pub(crate) struct PassthroughFs {
    disk_root: PathBuf,
    canonical_root: PathBuf,
}

impl PassthroughFs {
    pub fn new(disk_root: PathBuf) -> Self {
        let canonical_root = disk_root
            .canonicalize()
            .unwrap_or_else(|_| disk_root.clone());
        PassthroughFs {
            disk_root,
            canonical_root,
        }
    }

    fn real(&self, virtual_path: &str) -> PathBuf {
        to_real_path(&self.disk_root, virtual_path)
    }

    fn resolve_abs(&self, path: &str, cwd: &str) -> String {
        resolve_abs(path, cwd)
    }
}

impl FileSystem for PassthroughFs {
    fn resolve(&self, path: &str, cwd: &str) -> Option<()> {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        let canonical = real.canonicalize().ok()?;
        if canonical.starts_with(&self.canonical_root) {
            Some(())
        } else {
            None
        }
    }

    fn mkdir(&mut self, path: &str, cwd: &str) -> bool {
        let abs = self.resolve_abs(path, cwd);
        let (parent_path, name) = split_path(&abs);
        let parent_real = self.real(parent_path);
        let canonical_parent = match parent_real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !canonical_parent.starts_with(&self.canonical_root) {
            return false;
        }
        std::fs::create_dir(canonical_parent.join(name)).is_ok()
    }

    fn create_file(&mut self, path: &str, cwd: &str) -> bool {
        let abs = self.resolve_abs(path, cwd);
        let (parent_path, name) = split_path(&abs);
        let parent_real = self.real(parent_path);
        let canonical_parent = match parent_real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !canonical_parent.starts_with(&self.canonical_root) {
            return false;
        }
        let target = canonical_parent.join(name);
        if target.exists() {
            return false;
        }
        std::fs::write(&target, b"").is_ok()
    }

    fn read_file(&self, path: &str, cwd: &str) -> Option<Vec<u8>> {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        let canonical = real.canonicalize().ok()?;
        if !canonical.starts_with(&self.canonical_root) {
            return None;
        }
        std::fs::read(canonical).ok()
    }

    fn write_file(&mut self, path: &str, cwd: &str, data: &[u8]) -> bool {
        let abs = self.resolve_abs(path, cwd);
        // Try existing path first
        if let Ok(canonical) = self.real(&abs).canonicalize() {
            if !canonical.starts_with(&self.canonical_root) {
                return false;
            }
            if canonical.is_dir() {
                return false;
            }
            return std::fs::write(&canonical, data).is_ok();
        }
        // New file — validate parent
        let (parent_path, name) = split_path(&abs);
        let parent_real = self.real(parent_path);
        let canonical_parent = match parent_real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !canonical_parent.starts_with(&self.canonical_root) {
            return false;
        }
        let target = canonical_parent.join(name);
        if target.is_dir() {
            return false;
        }
        std::fs::write(&target, data).is_ok()
    }

    fn list_dir(&self, path: &str, cwd: &str, show_hidden: bool) -> Option<Vec<DirEntry>> {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        let canonical = real.canonicalize().ok()?;
        if !canonical.starts_with(&self.canonical_root) {
            return None;
        }
        let rd = std::fs::read_dir(&canonical).ok()?;
        let mut entries = Vec::new();
        for entry in rd.flatten() {
            if let Ok(name) = entry.file_name().into_string() {
                if !show_hidden && name.starts_with('.') {
                    continue;
                }
                let is_dir = entry.file_type().map_or(false, |ft| ft.is_dir());
                let size = if is_dir {
                    0
                } else {
                    entry.metadata().map_or(0, |m| m.len())
                };
                entries.push(DirEntry { name, is_dir, size });
            }
        }
        Some(entries)
    }

    fn remove(&mut self, path: &str, cwd: &str) -> bool {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        let canonical = match real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !canonical.starts_with(&self.canonical_root) {
            return false;
        }
        if canonical.is_dir() {
            return false;
        }
        std::fs::remove_file(&canonical).is_ok()
    }

    fn remove_all(&mut self, path: &str, cwd: &str) -> bool {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        let canonical = match real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !canonical.starts_with(&self.canonical_root) {
            return false;
        }
        if canonical.is_dir() {
            std::fs::remove_dir_all(&canonical).is_ok()
        } else {
            std::fs::remove_file(&canonical).is_ok()
        }
    }

    fn copy_file(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        let src_abs = self.resolve_abs(src, cwd);
        let dst_abs = self.resolve_abs(dst, cwd);

        let src_real = self.real(&src_abs);
        let src_canonical = match src_real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !src_canonical.starts_with(&self.canonical_root) {
            return false;
        }

        // Resolve destination (existing or new)
        let dst_target = if let Ok(canonical) = self.real(&dst_abs).canonicalize() {
            if !canonical.starts_with(&self.canonical_root) {
                return false;
            }
            if canonical.is_dir() {
                let name = fs_name(&src_abs);
                canonical.join(name)
            } else {
                canonical
            }
        } else {
            let (parent_path, name) = split_path(&dst_abs);
            let parent_real = self.real(parent_path);
            let canonical_parent = match parent_real.canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            if !canonical_parent.starts_with(&self.canonical_root) {
                return false;
            }
            canonical_parent.join(name)
        };

        std::fs::copy(&src_canonical, &dst_target).is_ok()
    }

    fn move_node(&mut self, src: &str, dst: &str, cwd: &str) -> bool {
        let src_abs = self.resolve_abs(src, cwd);
        let dst_abs = self.resolve_abs(dst, cwd);

        let src_real = self.real(&src_abs);
        let src_canonical = match src_real.canonicalize() {
            Ok(p) => p,
            Err(_) => return false,
        };
        if !src_canonical.starts_with(&self.canonical_root) {
            return false;
        }

        let dst_target = if let Ok(canonical) = self.real(&dst_abs).canonicalize() {
            if !canonical.starts_with(&self.canonical_root) {
                return false;
            }
            if canonical.is_dir() {
                let name = fs_name(&src_abs);
                canonical.join(name)
            } else {
                canonical
            }
        } else {
            let (parent_path, name) = split_path(&dst_abs);
            let parent_real = self.real(parent_path);
            let canonical_parent = match parent_real.canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            if !canonical_parent.starts_with(&self.canonical_root) {
                return false;
            }
            canonical_parent.join(name)
        };

        std::fs::rename(&src_canonical, &dst_target).is_ok()
    }

    fn exists(&self, path: &str, cwd: &str) -> bool {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        match real.canonicalize() {
            Ok(canonical) => canonical.starts_with(&self.canonical_root),
            Err(_) => false,
        }
    }

    fn is_dir(&self, path: &str, cwd: &str) -> bool {
        let abs = self.resolve_abs(path, cwd);
        let real = self.real(&abs);
        match real.canonicalize() {
            Ok(canonical) => canonical.starts_with(&self.canonical_root) && canonical.is_dir(),
            Err(_) => false,
        }
    }

    fn find(&self, start_path: &str, pattern: &str) -> Vec<String> {
        find_default(self, start_path, pattern)
    }
}

// ══════════════════════════════════════════════════════════════════
// Default find implementation (uses list_dir)
// ══════════════════════════════════════════════════════════════════

fn find_default(fs: &dyn FileSystem, start_path: &str, pattern: &str) -> Vec<String> {
    let start_path = if start_path.is_empty() {
        "/"
    } else {
        start_path
    };
    let abs = normalize_path(start_path);
    let abs = if abs.is_empty() { "/".to_string() } else { abs };
    let mut results = Vec::new();
    find_recursive(fs, &abs, pattern, &mut results, 0);
    results
}

fn find_recursive(
    fs: &dyn FileSystem,
    path: &str,
    pattern: &str,
    results: &mut Vec<String>,
    depth: usize,
) {
    if depth > 100 {
        return;
    }
    let filename = if path == "/" { "/" } else { fs_name(path) };
    if glob_matches(pattern, filename) {
        results.push(path.to_string());
    }
    if let Some(entries) = fs.list_dir(path, "/", true) {
        for entry in entries {
            let child_path = if path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", path, entry.name)
            };
            find_recursive(fs, &child_path, pattern, results, depth + 1);
        }
    }
}

// ══════════════════════════════════════════════════════════════════
// Glob matching
// ══════════════════════════════════════════════════════════════════

fn glob_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_inner(&p, &t, 0, 0, 0)
}

fn glob_inner(p: &[char], t: &[char], pi: usize, ti: usize, depth: usize) -> bool {
    if depth > 1000 {
        return false;
    }
    if pi == p.len() {
        return ti == t.len();
    }
    if pi >= p.len() || ti >= t.len() {
        return false;
    }
    match p[pi] {
        '*' => {
            for i in ti..=t.len() {
                if glob_inner(p, t, pi + 1, i, depth + 1) {
                    return true;
                }
            }
            false
        }
        '?' => {
            if ti < t.len() {
                glob_inner(p, t, pi + 1, ti + 1, depth + 1)
            } else {
                false
            }
        }
        '[' => {
            if ti >= t.len() {
                return false;
            }
            if let Some(end) = p[pi + 1..].iter().position(|&c| c == ']') {
                let class_end = pi + 1 + end;
                let chars: Vec<char> = p[pi + 1..class_end].to_vec();
                if chars.contains(&t[ti]) {
                    return glob_inner(p, t, class_end + 1, ti + 1, depth + 1);
                }
                false
            } else {
                t[ti] == '[' && glob_inner(p, t, pi + 1, ti + 1, depth + 1)
            }
        }
        c => t[ti] == c && glob_inner(p, t, pi + 1, ti + 1, depth + 1),
    }
}

// ══════════════════════════════════════════════════════════════════
// Tests
// ══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    // ── MemoryFs tests (regression) ───────────────────────────

    #[test]
    fn test_memory_basic() {
        let mut fs = Fs::new();
        assert!(fs.mkdir("/tmp", "/"));
        assert!(fs.write_file("/tmp/hello.txt", "/", b"world"));
        assert_eq!(fs.read_file("/tmp/hello.txt", "/"), Some(b"world".to_vec()));
        assert!(fs.is_dir("/tmp", "/"));
    }

    #[test]
    fn test_memory_relative() {
        let mut fs = Fs::new();
        fs.mkdir("/home", "/");
        fs.mkdir("/home/user", "/");
        fs.write_file("hello.txt", "/home/user", b"data");
        assert_eq!(
            fs.read_file("hello.txt", "/home/user"),
            Some(b"data".to_vec())
        );
    }

    #[test]
    fn test_memory_dotdot() {
        let mut fs = Fs::new();
        fs.mkdir("/a", "/");
        fs.mkdir("/a/b", "/");
        fs.write_file("/a/file.txt", "/", b"data");
        assert_eq!(fs.read_file("../file.txt", "/a/b"), Some(b"data".to_vec()));
    }

    #[test]
    fn test_memory_remove() {
        let mut fs = Fs::new();
        fs.write_file("/file.txt", "/", b"data");
        assert!(fs.exists("/file.txt", "/"));
        assert!(fs.remove("/file.txt", "/"));
        assert!(!fs.exists("/file.txt", "/"));
    }

    #[test]
    fn test_memory_remove_all() {
        let mut fs = Fs::new();
        fs.mkdir("/dir", "/");
        fs.write_file("/dir/a", "/", b"1");
        fs.write_file("/dir/b", "/", b"2");
        assert!(fs.remove_all("/dir", "/"));
        assert!(!fs.exists("/dir", "/"));
    }

    #[test]
    fn test_memory_copy() {
        let mut fs = Fs::new();
        fs.write_file("/src.txt", "/", b"hello");
        assert!(fs.copy_file("/src.txt", "/dst.txt", "/"));
        assert_eq!(fs.read_file("/dst.txt", "/"), Some(b"hello".to_vec()));
    }

    #[test]
    fn test_memory_move() {
        let mut fs = Fs::new();
        fs.write_file("/old.txt", "/", b"data");
        assert!(fs.move_node("/old.txt", "/new.txt", "/"));
        assert!(!fs.exists("/old.txt", "/"));
        assert_eq!(fs.read_file("/new.txt", "/"), Some(b"data".to_vec()));
    }

    #[test]
    fn test_memory_find() {
        let mut fs = Fs::new();
        fs.mkdir("/a", "/");
        fs.mkdir("/a/b", "/");
        fs.write_file("/a/b/c.txt", "/", b"");
        fs.write_file("/a/b/d.rs", "/", b"");
        let results = fs.find("/a", "*.txt");
        assert_eq!(results, vec!["/a/b/c.txt"]);
    }

    #[test]
    fn test_memory_list_dir() {
        let mut fs = Fs::new();
        fs.mkdir("/d", "/");
        fs.write_file("/d/a", "/", b"1");
        fs.write_file("/d/.hidden", "/", b"2");
        let entries = fs.list_dir("/d", "/", false).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a");
        let entries = fs.list_dir("/d", "/", true).unwrap();
        assert_eq!(entries.len(), 2);
    }

    // ── ReadThroughFs tests ───────────────────────────────────

    use std::fs as stdfs;
    use tempfile::tempdir;

    fn rt_fs(dir: &Path) -> Fs {
        Fs::with_mode(FsMode::ReadThrough(dir.to_path_buf()))
    }

    #[test]
    fn test_rt_read_from_disk() {
        let dir = tempdir().unwrap();
        stdfs::create_dir_all(dir.path().join("sub")).unwrap();
        stdfs::write(dir.path().join("sub/file.txt"), b"disk content").unwrap();

        let fs = rt_fs(dir.path());
        assert_eq!(
            fs.read_file("/sub/file.txt", "/"),
            Some(b"disk content".to_vec())
        );
    }

    #[test]
    fn test_rt_write_stays_in_memory() {
        let dir = tempdir().unwrap();
        let mut fs = rt_fs(dir.path());

        assert!(fs.write_file("/new.txt", "/", b"in-memory"));
        assert_eq!(fs.read_file("/new.txt", "/"), Some(b"in-memory".to_vec()));

        // File should NOT exist on real disk
        assert!(!dir.path().join("new.txt").exists());
    }

    #[test]
    fn test_rt_list_dir_merges() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("disk_file.txt"), b"x").unwrap();

        let mut fs = rt_fs(dir.path());
        fs.write_file("/mem_file.txt", "/", b"y");

        let entries = fs.list_dir("/", "/", false).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"disk_file.txt"));
        assert!(names.contains(&"mem_file.txt"));
    }

    #[test]
    fn test_rt_remove_hides_disk_entry() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("file.txt"), b"x").unwrap();

        let mut fs = rt_fs(dir.path());
        assert!(fs.remove("/file.txt", "/"));
        assert!(!fs.exists("/file.txt", "/"));

        let entries = fs.list_dir("/", "/", false).unwrap();
        assert!(entries.iter().all(|e| e.name != "file.txt"));
    }

    #[test]
    fn test_rt_mkdir() {
        let dir = tempdir().unwrap();
        let mut fs = rt_fs(dir.path());

        assert!(fs.mkdir("/mydir", "/"));
        assert!(fs.is_dir("/mydir", "/"));

        // Should NOT be on disk
        assert!(!dir.path().join("mydir").is_dir());
    }

    #[test]
    fn test_rt_write_does_not_modify_disk() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("existing.txt"), b"original").unwrap();

        let mut fs = rt_fs(dir.path());
        assert!(fs.write_file("/existing.txt", "/", b"modified"));

        // In-memory has the new content
        assert_eq!(
            fs.read_file("/existing.txt", "/"),
            Some(b"modified".to_vec())
        );
        // Disk still has original
        assert_eq!(
            stdfs::read(dir.path().join("existing.txt")).unwrap(),
            b"original"
        );
    }

    #[test]
    fn test_rt_find() {
        let dir = tempdir().unwrap();
        stdfs::create_dir_all(dir.path().join("src")).unwrap();
        stdfs::write(dir.path().join("src/main.rs"), b"").unwrap();
        stdfs::write(dir.path().join("README.md"), b"").unwrap();

        let fs = rt_fs(dir.path());
        let results = fs.find("/", "*.rs");
        assert!(results.contains(&"/src/main.rs".to_string()));
        assert!(!results.iter().any(|r| r.contains("README")));
    }

    // ── PassthroughFs tests ───────────────────────────────────

    fn pt_fs(dir: &Path) -> Fs {
        Fs::with_mode(FsMode::Passthrough(dir.to_path_buf()))
    }

    #[test]
    fn test_pt_write_and_read() {
        let dir = tempdir().unwrap();
        let mut fs = pt_fs(dir.path());

        assert!(fs.write_file("/hello.txt", "/", b"world"));
        assert_eq!(fs.read_file("/hello.txt", "/"), Some(b"world".to_vec()));

        // Should be on real disk
        assert_eq!(stdfs::read(dir.path().join("hello.txt")).unwrap(), b"world");
    }

    #[test]
    fn test_pt_mkdir() {
        let dir = tempdir().unwrap();
        let mut fs = pt_fs(dir.path());

        assert!(fs.mkdir("/mydir", "/"));
        assert!(fs.is_dir("/mydir", "/"));
        assert!(dir.path().join("mydir").is_dir());
    }

    #[test]
    fn test_pt_remove() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("file.txt"), b"x").unwrap();

        let mut fs = pt_fs(dir.path());
        assert!(fs.remove("/file.txt", "/"));
        assert!(!fs.exists("/file.txt", "/"));
        assert!(!dir.path().join("file.txt").exists());
    }

    #[test]
    fn test_pt_copy() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("src.txt"), b"hello").unwrap();

        let mut fs = pt_fs(dir.path());
        assert!(fs.copy_file("/src.txt", "/dst.txt", "/"));
        assert_eq!(stdfs::read(dir.path().join("dst.txt")).unwrap(), b"hello");
    }

    #[test]
    fn test_pt_move() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("old.txt"), b"data").unwrap();

        let mut fs = pt_fs(dir.path());
        assert!(fs.move_node("/old.txt", "/new.txt", "/"));
        assert!(!dir.path().join("old.txt").exists());
        assert_eq!(stdfs::read(dir.path().join("new.txt")).unwrap(), b"data");
    }

    #[test]
    fn test_pt_list_dir() {
        let dir = tempdir().unwrap();
        stdfs::write(dir.path().join("a"), b"").unwrap();
        stdfs::write(dir.path().join(".hidden"), b"").unwrap();

        let fs = pt_fs(dir.path());
        let entries = fs.list_dir("/", "/", false).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a");

        let entries = fs.list_dir("/", "/", true).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_rt_remove_all() {
        let dir = tempdir().unwrap();
        stdfs::create_dir_all(dir.path().join("a/b")).unwrap();
        stdfs::write(dir.path().join("a/b/c.txt"), b"x").unwrap();

        let mut fs = rt_fs(dir.path());
        assert!(fs.remove_all("/a", "/"));
        assert!(!fs.exists("/a", "/"));
        assert!(!fs.exists("/a/b", "/"));
        assert!(!fs.exists("/a/b/c.txt", "/"));

        // Disk still has the files
        assert!(dir.path().join("a/b/c.txt").exists());
    }

    #[test]
    fn test_pt_find() {
        let dir = tempdir().unwrap();
        stdfs::create_dir_all(dir.path().join("d")).unwrap();
        stdfs::write(dir.path().join("d/a.txt"), b"").unwrap();
        stdfs::write(dir.path().join("d/b.rs"), b"").unwrap();
        stdfs::write(dir.path().join("d/c.txt"), b"").unwrap();

        let fs = pt_fs(dir.path());
        let results = fs.find("/d", "*.txt");
        assert!(results.contains(&"/d/a.txt".to_string()));
        assert!(results.contains(&"/d/c.txt".to_string()));
        assert!(!results.iter().any(|r| r.contains("b.rs")));
    }

    // ── Glob tests ────────────────────────────────────────────

    #[test]
    fn test_glob() {
        assert!(glob_matches("*.txt", "hello.txt"));
        assert!(!glob_matches("*.txt", "hello.rs"));
        assert!(glob_matches("hello.*", "hello.txt"));
        assert!(glob_matches("h?llo", "hello"));
        assert!(!glob_matches("h?llo", "hllo"));
        assert!(glob_matches("[abc].txt", "a.txt"));
        assert!(!glob_matches("[abc].txt", "d.txt"));
        assert!(glob_matches("*", "anything"));
    }

    // ── Security tests ──────────────────────────────────────────

    #[test]
    fn test_pt_symlink_escape_read() {
        let dir = tempdir().unwrap();
        let secret_dir = tempdir().unwrap();
        stdfs::write(secret_dir.path().join("secret.txt"), b"SENSITIVE").unwrap();

        // Create symlink inside sandbox pointing outside
        #[cfg(unix)]
        symlink(secret_dir.path(), dir.path().join("escape")).unwrap();

        let fs = pt_fs(dir.path());
        // Reading through the symlink should fail
        assert_eq!(fs.read_file("/escape/secret.txt", "/"), None);
    }

    #[test]
    fn test_pt_symlink_escape_write() {
        let dir = tempdir().unwrap();
        let outside_dir = tempdir().unwrap();

        #[cfg(unix)]
        symlink(outside_dir.path(), dir.path().join("escape")).unwrap();

        let mut fs = pt_fs(dir.path());
        // Writing through the symlink should fail
        assert!(!fs.write_file("/escape/evil.txt", "/", b"pwned"));
        // Confirm nothing was written outside
        assert!(!outside_dir.path().join("evil.txt").exists());
    }

    #[test]
    fn test_pt_symlink_escape_mkdir() {
        let dir = tempdir().unwrap();
        let outside_dir = tempdir().unwrap();

        #[cfg(unix)]
        symlink(outside_dir.path(), dir.path().join("escape")).unwrap();

        let mut fs = pt_fs(dir.path());
        assert!(!fs.mkdir("/escape/evil", "/"));
        assert!(!outside_dir.path().join("evil").exists());
    }

    #[test]
    fn test_pt_symlink_escape_remove() {
        let dir = tempdir().unwrap();
        let outside_dir = tempdir().unwrap();
        stdfs::write(outside_dir.path().join("important.txt"), b"data").unwrap();

        #[cfg(unix)]
        symlink(outside_dir.path(), dir.path().join("escape")).unwrap();

        let mut fs = pt_fs(dir.path());
        assert!(!fs.remove("/escape/important.txt", "/"));
        // File should still exist outside
        assert!(outside_dir.path().join("important.txt").exists());
    }

    #[test]
    fn test_rt_symlink_escape_read() {
        let dir = tempdir().unwrap();
        let secret_dir = tempdir().unwrap();
        stdfs::write(secret_dir.path().join("secret.txt"), b"SENSITIVE").unwrap();

        #[cfg(unix)]
        symlink(secret_dir.path(), dir.path().join("escape")).unwrap();

        let fs = rt_fs(dir.path());
        assert_eq!(fs.read_file("/escape/secret.txt", "/"), None);
    }

    #[test]
    fn test_memoryfs_file_size_limit() {
        let limits = FsLimits {
            max_file_size: 1024, // 1KB limit
            ..Default::default()
        };
        let mut fs = Fs::with_limits(limits);

        // Small file should work
        assert!(fs.write_file("/small.txt", "/", &[0u8; 512]));

        // Oversized file should be rejected
        assert!(!fs.write_file("/big.txt", "/", &[0u8; 2048]));

        // Existing file overwrite with oversized data should be rejected
        assert!(!fs.write_file("/small.txt", "/", &[0u8; 2048]));
    }

    #[test]
    fn test_memoryfs_dir_entry_limit() {
        let limits = FsLimits {
            max_dir_entries: 3,
            ..Default::default()
        };
        let mut fs = Fs::with_limits(limits);

        assert!(fs.mkdir("/dir", "/"));
        assert!(fs.write_file("/dir/a", "/", b"1"));
        assert!(fs.write_file("/dir/b", "/", b"2"));
        assert!(fs.write_file("/dir/c", "/", b"3"));
        // Fourth entry should be rejected
        assert!(!fs.write_file("/dir/d", "/", b"4"));
        assert!(!fs.mkdir("/dir/subdir", "/"));
    }

    #[test]
    fn test_memoryfs_total_node_limit() {
        let limits = FsLimits {
            max_total_nodes: 3, // root + 2 more
            ..Default::default()
        };
        let mut fs = Fs::with_limits(limits);

        assert!(fs.mkdir("/a", "/"));
        assert!(fs.write_file("/b", "/", b"data"));
        // Third new node should fail
        assert!(!fs.mkdir("/c", "/"));
    }

    #[test]
    fn test_deep_tree_remove_no_stack_overflow() {
        let mut fs = Fs::new();
        // Create a deep tree (10000 levels)
        let mut path = String::from("/deep");
        fs.mkdir(&path, "/");
        for _ in 0..9999 {
            path.push_str("/x");
            fs.mkdir(&path, "/");
        }
        // This should not stack overflow
        assert!(fs.remove_all("/deep", "/"));
        assert!(!fs.exists("/deep", "/"));
    }

    #[test]
    fn test_glob_no_exponential_backtracking() {
        // Pattern with many wildcards against adversarial text
        // Without depth limit, this would be exponential
        let pattern = "*********a";
        let text = "aaaaaaaaaab";
        // Should return quickly (either true or false), not hang
        assert!(!glob_matches(pattern, text));
    }
}
