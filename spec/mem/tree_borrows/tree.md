# Tree Structure in Tree Borrows

The core data structure of Tree Borrows is a *tree*, with a state machine in each node.
We use a tree to track reborrows in each allocation; each reborrow adds a new node to the tree.
The per-node state machine is defined in [state_machine.md](state_machine.md).

When a reborrow occurs at function entry (i.e. the original reference is passed as an argument), we add a *protector* to the reborrow.
When a node is protected, it is UB for this node to become `Disabled`.
There are two types of protectors: *strong* and *weak*. A *strong* protector corresponds to a normal reference, while a *weak* protector corresponds to a `Box`.
The key difference is that weak protectors still permit the memory to be deallocated, while it is UB to deallocate memory as long as there is any strongly protected node.
We use the following enum to represent whether the node is protected or not, and when it is protected, what type the protector is.

```rust
enum Protected {
    Strong,
    Weak,
    No,
}

impl Protected {
    /// Check whether the node is either strongly or weakly protected.
    fn yes(self) -> bool {
        self != Protected::No
    }
}
```

Then we can define the node. Structurally, we use the usual functional representation of a tree: we store a list of children.

```rust
struct Node {
    children: List<Node>,
    /// State for each location
    location_states: List<LocationState>,
    /// Indicates whether the node is protected by a function call,
    /// i.e., whether the original reference passed as an argument of a function call.
    /// This will be some kind of protection (weak or strong) if and only if this node is in
    /// some frame's `extra.protectors` list.
    protected: Protected,
}
```

During each memory access, we update states according to the state machine.
When a node is accessed, each node in the tree can be divided into two disjoint sets: *local nodes* and *foreign nodes*, based on its relative position to the accessed node.
The *local* set includes the node itself and all its descendants, while the *foreign* set contains all other nodes.

```rust
enum NodeRelation {
    Local,
    Foreign,
}
```

The state transition depends on both the node relation and the type of access operation.

```rust
enum AccessKind {
    Read, 
    Write,
}
```

Then we implement how to actually update and check states in a Tree during each memory access.

```rust
impl Node {
    /// Perform state transition on all locations of `self`.
    /// The `node_relation` indicates whether the accessed node is a local or foreign
    /// *from the perspective of `self`*.
    fn transition(
        &mut self, 
        node_relation: NodeRelation,
        access_kind: AccessKind,
        offset_in_alloc: Offset,
        size: Size,
    ) -> Result {
        let offset_start = offset_in_alloc.bytes();
        for offset in offset_start..offset_start + size.bytes() {
            self.location_states.mutate_at(offset, |location_state|{
                location_state.transition(access_kind, node_relation, self.protected.yes())
            })?;
        }

        ret(())
    }

    /// Recusively do state transition on `self` node and all its descendants.
    /// `path` is the path from `self` to the accessed node.
    /// `path` is None when the accessed node is not a descendant of `self`.
    ///
    /// This method will throw UBs, representing the undefined behavior captured by Tree Borrows.
    fn access(
        &mut self,
        path: Option<Path>, // self -> accessed node
        access_kind: AccessKind,
        offset_in_alloc: Offset,
        size: Size,
    ) -> Result {
        // Indicates whether the accessed node is a local or foreign
        // *from the perspective of `self`*.
        //
        // If `self` is an ancestor of the accessed node, the accessed node is local to `self`.
        // Otherwise, the accessed node is a `foreign` of `self`.
        let node_relation = if path.is_some() { NodeRelation::Local } else { NodeRelation::Foreign };

        // Perform state transition on `self`.
        self.transition(node_relation, access_kind, offset_in_alloc, size)?;

        for child_id in Int::ZERO..self.children.len() {
            // If the path starts with this child, do a child access with the path shortened by the first element.
            // Otherwise, do a child access with the path being None (i.e., child is not an ancestor of accessed node)
            let sub_path = match path.and_then(|p| p.split_first()) {
                Some((head, tail)) if head == child_id => Some(tail),
                _ => None,
            };

            self.children.mutate_at(child_id, |child| {
                child.access(sub_path, access_kind, offset_in_alloc, size)
            })?;
        }

        ret(())
    }
}
```

We also implement some helper methods for manipulating Trees.

```rust
impl Node {
    /// Apply `f` to a child node of `self`
    /// `path` is the path from `self` to the target child node.
    fn access_node<O>(&mut self, path: Path, f: impl FnOnce(&mut Self) -> O) -> O {
        // `sub_root_id` indicates which node to access next; call it the sub-root.
        // `sub_path` is the path from the sub-root to the target node.
        let Some((sub_root_id, sub_path)) = path.split_first() else {
            return f(self);
        };

        // If `self` is a leaf and the path keeps going, then the path is invalid.
        if self.is_leaf() { panic!("Node::access_node: invalid node path"); }

        // Find the right child, and recursively search for the target node.
        self.children.mutate_at(sub_root_id, |child| child.access_node(sub_path, f))
    }

    /// Add a new child node to the tree whose root is self
    /// `path` is the path from `self` to the parent of the `node`.
    ///
    /// Return the path from `self` to the `node`
    fn add_node(&mut self, parent_path: Path, child: Node) -> Path {
        let child_idx = self.access_node(parent_path, |node| {
            let child_idx = node.children.len();
            node.children.push(child);
            child_idx
        });

        let mut child_path = parent_path;
        child_path.push(child_idx);

        child_path
    }

    /// Get a child node of `self`
    /// `path` is the path from `self` to the target child node.
    fn get_node(&mut self, path: Path) -> Node {
        self.access_node(path, |node| *node)
    }

    /// Check whether `self` is a leaf.
    fn is_leaf(&self) -> bool {
        self.children.len() == Int::ZERO
    }
}
```

In addition, we implement some methods for protector-related semantics.
```rust
impl Node {
    /// Release the protector, and perform a special access for the protector end semantics.
    /// Recusively do state transition on all foreigns of the protected node.
    /// `path` is the path from `self` to the proctected node.
    /// `path` is None when the protected node is not a descendant of `self`.
    /// `location_states` are the location states of the protected node.
    fn release_protector(
        &mut self,
        path: Option<Path>, // self -> protected node
        location_states: &List<LocationState>,
    ) -> Result {
        // Indicates whether the protected node is a local or foreign
        // *from the perspective of `self`*.
        let node_relation = if path.is_some() { NodeRelation::Local } else { NodeRelation::Foreign };

        // If `self` is the protected node, we are done: the special access
        // does not apply to children of the protected node.
        if path.is_some_and(|p| p.is_empty()) {
            self.protected = Protected::No;
            return ret(());
        }

        for offset in Int::ZERO..location_states.len() {
            let LocationState { accessed, permission } = location_states[offset];

            // If the location has never been accessed, there is no need to perform an access here.
            if accessed != Accessed::Yes { continue; }

            // If the permission is Unique,
            // we perform a write access here. Otherwise, we perform a read access here.
            // Note that since this implicit access only occurs with actively protected nodes,
            // a foreign read/write of an Unique location should be UB.
            // This condition is hence equivalent to checking whether there was a (local) write to this location.
            let access_kind = match permission {
                Permission::Unique => AccessKind::Write,
                _ => AccessKind::Read,
            };

            self.location_states.mutate_at(Int::from(offset), |location_state|{
                // Perform state transition on `self`.
                location_state.transition(access_kind, node_relation, self.protected.yes())
            })?;
        }

        // Recursively visit children.
        for child_id in Int::ZERO..self.children.len() {
            let sub_path = match path.and_then(|p| p.split_first()) {
                Some((head, tail)) if head == child_id => Some(tail),
                _ => None,
            };

            self.children.mutate_at(child_id, |child| { child.release_protector(sub_path, &location_states) })?;
        }

        ret(())
    }

    /// Recusively check whether there is a strongly protected node in `self` and all its descendants.
    /// This is used to reject deallocation as long as there's a strong protector anywhere.
    ///
    /// Return true if there is a strongly protected node.
    fn contains_strong_protector(&self) -> bool {
        self.protected == Protected::Strong || self.children.any(|child| child.contains_strong_protector())
    }
}
```
