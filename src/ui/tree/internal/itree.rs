//! Internal tree structure implementation: the `Itree` structure.

use std::sync::Arc;
use std::{collections::VecDeque, iter::Iterator};

use crate::ui::tree::internal::inode::{Inode, InodePtr};

#[derive(Debug, Clone)]
pub struct Itree<T> {
  root: Option<InodePtr<T>>,
}

#[derive(Debug, Clone)]
/// The pre-order iterator of the tree.
///
/// It iterates the tree nodes following the order of rendering, i.e. the nodes with lower z-index
/// that can be covered by other nodes are visited earlier, the nodes with higher z-index that will
/// cover other nodes are visited later.
pub struct ItreeIterator<T> {
  order: ItreeIterateOrder,
  queue: VecDeque<InodePtr<T>>,
}

impl<T> Iterator for ItreeIterator<T> {
  type Item = InodePtr<T>;

  fn next(&mut self) -> Option<Self::Item> {
    if let Some(node) = self.queue.pop_front() {
      match node.read().unwrap().children() {
        Some(children) => match self.order {
          ItreeIterateOrder::Ascent => {
            for child in children.iter() {
              self.queue.push_back(child.clone());
            }
          }
          ItreeIterateOrder::Descent => {
            for child in children.iter().rev() {
              self.queue.push_back(child.clone());
            }
          }
        },
        None => { /* Do nothing */ }
      }
      return Some(node);
    }
    None
  }
}

impl<T> ItreeIterator<T> {
  pub fn new(start: Option<InodePtr<T>>, order: ItreeIterateOrder) -> Self {
    let mut queue = VecDeque::new();
    match start {
      Some(start_node) => queue.push_back(start_node),
      None => { /* Do nothing */ }
    }
    ItreeIterator { order, queue }
  }
}

#[derive(Debug, Clone)]
/// The iterator's visiting order for all children nodes under the same node.
///
/// The `Ascent` visits from lower z-index to higher.
/// The `Descent` visits from higher z-index to lower.
pub enum ItreeIterateOrder {
  Ascent,
  Descent,
}

impl<T> Itree<T> {
  pub fn new() -> Self {
    Itree { root: None }
  }

  pub fn is_empty(&self) -> bool {
    self.root.is_none()
  }

  pub fn root(&self) -> Option<InodePtr<T>> {
    match &self.root {
      Some(root) => Some(root.clone()),
      None => None,
    }
  }

  pub fn set_root(&mut self, root: Option<InodePtr<T>>) -> Option<InodePtr<T>> {
    let old = match &self.root {
      Some(root) => Some(root.clone()),
      None => None,
    };
    self.root = root;
    old
  }

  /// Assert the `node` exists in the tree.
  ///
  /// # Panics
  ///
  /// Panics when the `node` doesn't exist.
  fn assert_exists(&self, node: InodePtr<T>) {
    assert!(
      self.root.is_some(),
      "Doesn't have a root node when assert the node exists"
    );
    let node = node.write().unwrap();
    let node2 = self
      .root
      .clone()
      .unwrap()
      .write()
      .unwrap()
      .get_descendant(node.id());
    assert!(node2.is_some(), "Missing node {} in the tree", node.id());
    assert!(
      node2.unwrap().read().unwrap().id() == node.id(),
      "Node ID {} not match in the tree",
      node.id()
    );
  }

  /// Assert the `node` is the root node.
  ///
  /// # Panics
  ///
  /// Panics if the `node` isn't the root node.
  fn assert_is_root(&self, node: InodePtr<T>) {}

  /// Assert the `node` is not the root node, but exists in the tree.
  ///
  /// # Panics
  ///
  /// Panics if the `node` is the root node.
  fn assert_not_root(&self, node: InodePtr<T>) {}

  /// Get the iterator.
  ///
  /// By default it iterates in pre-order, start from the root. For the children under the same
  /// node, it visits from lower z-index to higher.
  pub fn iter(&self) -> ItreeIterator<T> {
    ItreeIterator::new(self.root.clone(), ItreeIterateOrder::Ascent)
  }

  /// Get the iterator with specified order.
  pub fn ordered_iter(&self, order: ItreeIterateOrder) -> ItreeIterator<T> {
    ItreeIterator::new(self.root, order)
  }

  /// Insert a child node into the parent node.
  ///
  /// Note:
  /// 1. When no parent node is provided, the node is inserted as the root node of the tree.
  /// 2. When a parent node is provided, the node is inserted as the child node of the parent node,
  ///    you need to get the parent node before insert.
  pub fn insert(&mut self, parent: Option<InodePtr<T>>, child: InodePtr<T>) -> Option<InodePtr<T>> {
    match parent {
      Some(parent) => {
        self.assert_exists(parent.clone());
        child
          .write()
          .unwrap()
          .set_parent(Some(Arc::downgrade(&parent)));
        Inode::push(parent, child.clone());
        Some(child.clone())
      }
      None => {
        assert!(
          self.root.is_none(),
          "Root node already exists when inserting without parent"
        );
        self.root = Some(child.clone());
        Some(child.clone())
      }
    }
  }

  /// Get a node by its ID.
  pub fn get(&self, id: usize) -> Option<InodePtr<T>> {
    match self.root.clone() {
      Some(root) => root.read().unwrap().get_descendant(id),
      None => None,
    }
  }

  /// Remove a node from the parent node.
  pub fn remove(parent: InodePtr<T>, index: usize) -> Option<InodePtr<T>> {
    parent.write().unwrap().remove(index)
  }
}
