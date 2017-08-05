use std::cmp::{max, min};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, Range, Sub};

use take_mut;

use util::WithSideEffects;


#[derive(Debug)]
pub struct ScheduleTree<'a, T, D: 'a + Eq + Hash> {
    root: Option<Node<'a, T, D>>,
    scope: Option<Range<T>>,
    data_map: HashMap<&'a D, T>,
}


#[derive(Debug, PartialEq)]
pub enum Node<'a, T, D: 'a> {
    Leaf { start: T, end: T, data: &'a D },
    Intermediate {
        free: Range<T>,
        left: Box<Node<'a, T, D>>,
        right: Box<Node<'a, T, D>>,
    },
}


impl<'a, T, D> ScheduleTree<'a, T, D>
    where T: Copy + Ord + Debug,
          D: Debug + Eq + Hash
{
    /// Returns an empty schedule tree.
    pub fn new() -> Self {
        ScheduleTree {
            root: None,
            scope: None,
            data_map: HashMap::new(),
        }
    }

    /// Returns a chronological iterator of the schedule tree.
    pub fn iter<'b>(&'b self) -> Iter<'b, 'a, T, D> {
        Iter { path: self.root.iter().collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Tries to schedule `data` at the exact `start` with the given `duration`.
    ///
    /// Returns whether the scheduling succeeded.
    #[allow(dead_code)]
    pub fn schedule_exact<W>(&mut self, start: T, duration: W, data: &'a D) -> bool
        where T: Add<W, Output = T>
    {
        self.schedule_exact_(start, duration, data)
            .with_side_effects(|start| self.update_map(start, data))
            .is_some()
    }

    /// See `schedule_exact` for details.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn schedule_exact_<W>(&mut self, start: T, duration: W, data: &'a D) -> Option<T>
        where T: Add<W, Output = T>
    {
        let end = start + duration;
        return_on_some!(self.try_schedule_trivial_cases(start, end, data));

        self.root.as_mut().unwrap().insert(start, end, data)
    }

    /// Tries to schedule `data` as close as possible before `end` with the given `duration`. It
    /// must be scheduled after `min_start` when given.
    ///
    /// Returns whether the scheduling succeeded.
    pub fn schedule_close_before<W>(&mut self, end: T, duration: W, min_start: Option<T>, data: &'a D) -> bool
        where T: Add<W, Output = T> + Sub<W, Output = T>,
              W: Copy + Debug
    {
        self.schedule_close_before_(end, duration, min_start, data)
            .with_side_effects(|start| self.update_map(start, data))
            .is_some()
    }

    /// See `schedule_close_before` for details.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn schedule_close_before_<W>(&mut self, end: T, duration: W, min_start: Option<T>, data: &'a D) -> Option<T>
        where T: Add<W, Output = T> + Sub<W, Output = T>,
              W: Copy + Debug
    {
        assert!(min_start.map_or(true, |min_start| min_start + duration <= end));

        let optimal_start = end - duration;
        return_on_some!(self.try_schedule_trivial_cases(optimal_start, end, data));

        return_on_some!(self.root.as_mut().unwrap()
                        .insert_before(end, duration, min_start, data));

        // As last resort, try to schedule before current scope if min_start allows
        let scope = self.scope.as_ref().cloned().unwrap();
        if min_start.map_or(true, |min_start| min_start <= scope.start - duration) {
            // Schedule on [scope.start - duration, scope.start]
            let start = scope.start - duration;
            let end = scope.start;
            let new_node = Node::Leaf {
                start: start,
                end: end,
                data: data,
            };
            self.root = Some(Node::Intermediate {
                                 left: Box::new(new_node),
                                 right: Box::new(self.root.take().unwrap()),
                                 free: scope.start..scope.start,
                             });
            self.scope = Some(start..scope.end);
            return Some(start)
        }

        None
    }

    /// Tries to schedule `data` as close as possible after `start` with the given `duration`. It
    /// must be scheduled before `max_end` when given.
    ///
    /// Returns whether the scheduling succeeded.
    pub fn schedule_close_after<W>(&mut self, start: T, duration: W, max_end: Option<T>, data: &'a D) -> bool
        where T: Add<W, Output = T> + Sub<W, Output = T>,
              W: Copy + Debug
    {
        self.schedule_close_after_(start, duration, max_end, data)
            .with_side_effects(|start| self.update_map(start, data))
            .is_some()
    }

    /// See `schedule_close_after` for details.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn schedule_close_after_<W>(&mut self, start: T, duration: W, max_end: Option<T>, data: &'a D) -> Option<T>
        where T: Add<W, Output = T> + Sub<W, Output = T>,
              W: Copy + Debug
    {
        assert!(max_end.map_or(true, |max_end| start + duration <= max_end));

        let optimal_end = start + duration;
        return_on_some!(self.try_schedule_trivial_cases(start, optimal_end, data));

        return_on_some!(self.root.as_mut().unwrap()
                        .insert_after(start, duration, max_end, data));

        // As last resort, try to schedule after current scope if max_end allows
        let scope = self.scope.as_ref().cloned().unwrap();
        if max_end.map_or(true, |max_end| scope.end + duration <= max_end) {
            // Schedule on [scope.end, scope.end + duration]
            let start = scope.end;
            let end = scope.end + duration;
            let new_node = Node::Leaf {
                start: start,
                end: end,
                data: data,
            };
            self.root = Some(Node::Intermediate {
                                 left: Box::new(self.root.take().unwrap()),
                                 right: Box::new(new_node),
                                 free: scope.end..scope.end,
                             });
            self.scope = Some(scope.start..end);
            return Some(start)
        }

        None
    }

    /// Common scheduling cases between all scheduling strategies. It handles the cases where
    /// (a) the schedule tree is empty;
    /// (b) the most optimal start and end fall completely before the left-most child in the tree
    /// (c) the most optimal start and end fall completely after the right-most child in the tree
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn try_schedule_trivial_cases(&mut self, start: T, end: T, data: &'a D) -> Option<T> {
        let new_node = Node::Leaf {
            start: start,
            end: end,
            data: data,
        };

        if self.root.is_none() {
            self.root = Some(new_node);
            self.scope = Some(start..end);
            return Some(start)
        }

        let scope = self.scope.as_ref().cloned().unwrap();
        if end <= scope.start {
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                                 left: Box::new(new_node),
                                 right: Box::new(root),
                                 free: end..scope.start,
                             });
            self.scope = Some(start..scope.end);
            return Some(start)
        } else if scope.end <= start {
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                                 left: Box::new(root),
                                 right: Box::new(new_node),
                                 free: scope.end..start,
                             });
            self.scope = Some(scope.start..end);
            return Some(start)
        }

        None
    }

    /// Removes the given data from the schedule tree.
    ///
    /// Returns the related entry from the tree if the tree contained it, otherwise None.
    pub fn unschedule(&mut self, data: &'a D) -> Option<Entry<'a, T, D>> {
        let when = self.remove_from_map(data);
        match (self.root.take(), when) {
            (Some(mut root), Some(when)) => {
                match root {
                    Node::Leaf { start, end, data } => {
                        self.root = None;
                        self.scope = None;
                        Some(Entry { start, end, data })
                    },
                    Node::Intermediate { .. } => {
                        let entry = root.unschedule(when, data)
                            .map(|(entry, scope)| {
                                self.scope = Some(scope);
                                entry
                            });
                        self.root = Some(root);
                        entry
                    },
                }
            },
            _ => None,
        }
    }

    pub fn when_scheduled(&self, data: &'a D) -> Option<&T> {
        self.data_map.get(data)
    }

    fn remove_from_map(&mut self, data: &'a D) -> Option<T> {
        self.data_map.remove(data)
    }

    fn update_map(&mut self, start: T, data: &'a D) {
        let old_value = self.data_map.insert(data, start);
        if old_value.is_some() {
            panic!("Internal error: same data is being entered twice.")
        }
    }
}


impl<'a, T, D> Node<'a, T, D>
    where T: Copy + Ord + Debug,
          D: Debug
{
    /// Tries to insert a node with given `start`, `end` and `data` as a descendant of this node.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn insert(&mut self, start: T, end: T, data: &'a D) -> Option<T> {
        match *self {
            Node::Leaf { .. } => None,
            Node::Intermediate {
                ref mut left,
                ref mut right,
                ref mut free,
            } => {
                if end <= free.start {
                    left.insert(start, end, data)
                } else if free.end <= start {
                    right.insert(start, end, data)
                } else if free.start <= start && end <= free.end {
                    // [start, end] completely within self.free
                    unchecked_insert(start, end, data, right, free);
                    Some(start)
                } else {
                    // Overlap between [start, end] and self.free
                    None
                }
            }
        }
    }

    /// Tries to insert a node with the given `data` and `duration` as a descendant of this node.
    /// It must be scheduled as close before `end` as possible, but it cannot be scheduled sooner
    /// than `min_start`, when given.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn insert_before<W>(&mut self, end: T, duration: W, min_start: Option<T>, data: &'a D) -> Option<T>
        where T: Sub<W, Output = T>,
              W: Copy + Debug
    {
        match *self {
            Node::Leaf { .. } => None,
            Node::Intermediate {
                ref mut left,
                ref mut right,
                ref mut free,
            } => {
                // If the end is inside the right child, try that first
                if free.end < end {
                    match right.insert_before(end, duration, min_start, data) {
                        Some(start) => return Some(start),
                        None => ()
                    }
                }
                // Second, try to insert it in the free range of the current node
                let end = min(end, free.end);
                if free.start <= end - duration {
                    if min_start.map_or(true, |min_start| min_start <= end - duration) {
                        unchecked_insert(end - duration, end, data, right, free);
                        return Some(end - duration)
                    }
                }

                // If min_start is contained in free, don't bother checking the left child
                if min_start.map_or(true, |min_start| free.start <= min_start) {
                    return None
                }
                // Last, try to insert it in the left child
                left.insert_before(end, duration, min_start, data)
            }
        }
    }

    /// Tries to insert a node with the given `data` and `duration` as a descendant of this node.
    /// It must be scheduled as close after `start` as possible, but it cannot be scheduled later
    /// than `max_end`, when given.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn insert_after<W>(&mut self, start: T, duration: W, max_end: Option<T>, data: &'a D) -> Option<T>
        where T: Ord + Add<W, Output = T>,
              W: Copy + Debug
    {
        match *self {
            Node::Leaf { .. } => None,
            Node::Intermediate {
                ref mut left,
                ref mut right,
                ref mut free,
            } => {
                // If the start is inside the left child, try that first
                if start < free.start {
                    match left.insert_after(start, duration, max_end, data) {
                        Some(start) => return Some(start),
                        None => ()
                    }
                }
                // Second, try to insert it in the free range of the current node
                let start = max(start, free.start);
                if start + duration <= free.end {
                    if max_end.map_or(true, |max_end| start + duration <= max_end) {
                        unchecked_insert(start, start + duration, data, right, free);
                        return Some(start)
                    }
                }
                // If max_end is contained in free, don't bother checking the right child
                if max_end.map_or(true, |max_end| max_end <= free.end) {
                    return None
                }
                // Last, try to insert it in the right child
                right.insert_after(start, duration, max_end, data)
            }
        }
    }

    /// Tries to unschedule certain data a certain start.
    ///
    /// Returns None if that combination wasn't found, otherwise a tuple of an entry representing
    /// the unscheduled item and the new scope of this node.
    fn unschedule(&mut self, start: T, data: &'a D) -> Option<(Entry<'a, T, D>, Range<T>)>
        where D: PartialEq
    {
        match *self {
            Node::Leaf { .. } => panic!("`unschedule` called on a leaf node"),
            Node::Intermediate { .. } => {
                if start < self.unchecked_free_ref().start {
                    match *self.unchecked_left_mut() {
                        Node::Leaf { start: node_start, end, data: node_data } => {
                            if start == node_start && data == node_data {
                                take_mut::take(self, |self_| self_.unchecked_right());
                                Some((Entry { start: start, end: end, data: data },
                                      self.find_scope()))
                            } else {
                                None
                            }
                        },
                        Node::Intermediate { .. } => {
                            self.unchecked_left_mut().unschedule(start, data)
                                .map(|(entry, scope)| {
                                    self.unchecked_free_mut().start = scope.end;
                                    (entry, self.find_scope()) // FIXME too naive
                                })
                        },
                    }
                } else if self.unchecked_free_ref().end <= start {
                    match *self.unchecked_right_mut() {
                        Node::Leaf { start: node_start, end, data: node_data } => {
                            if start == node_start && data == node_data {
                                take_mut::take(self, |self_| self_.unchecked_left());
                                Some((Entry { start: start, end: end, data: data },
                                      self.find_scope()))
                            } else {
                                None
                            }
                        },
                        Node::Intermediate { .. } => {
                            self.unchecked_right_mut().unschedule(start, data)
                                .map(|(entry, scope)| {
                                    self.unchecked_free_mut().end = scope.start;
                                    (entry, self.find_scope()) // FIXME too naive
                                })
                        },
                    }
                } else {
                    None
                }
            }
        }
    }

    /// Calculates the scope of all descendants of this node.
    fn find_scope(&self) -> Range<T> {
        match *self {
            Node::Leaf { start, end, .. } => start..end,
            Node::Intermediate { ref left, ref right, .. } => {
                let start = left.find_scope().start;
                let end = right.find_scope().end;
                start..end
            },
        }
    }

    // From here on there will be a bunch of helper methods, because either I don't understand Rust
    // well enough or because destructuring enums and borrowing doesn't work well together and this
    // is the only way to overcome that. If enum variants were types, this would all be solved, I
    // think. IIRC, there is an old RFC for that.
    //
    // At the moment, they are only necessary for `unschedule`.

    /// Assume `self` is an intermediate node and return a mutable reference to the left child.
    fn unchecked_left_mut(&mut self) -> &mut Node<'a, T, D> {
        match *self {
            Node::Leaf { .. } => panic!("`unchecked_left_mut` called on a leaf node"),
            Node::Intermediate { ref mut left, .. } => left,
        }
    }

    /// Assume `self` is an intermediate node and return a mutable reference to the right child.
    fn unchecked_right_mut(&mut self) -> &mut Node<'a, T, D> {
        match *self {
            Node::Leaf { .. } => panic!("`unchecked_right_mut` called on a leaf node"),
            Node::Intermediate { ref mut right, .. } => right,
        }
    }

    /// Assume `self` is an intermediate node and return the left child.
    fn unchecked_left(self) -> Node<'a, T, D> {
        match self {
            Node::Leaf { .. } => panic!("`unchecked_left` called on a leaf node"),
            Node::Intermediate { box left, .. } => left,
        }
    }

    /// Assume `self` is an intermediate node and return the right child.
    fn unchecked_right(self) -> Node<'a, T, D> {
        match self {
            Node::Leaf { .. } => panic!("`unchecked_right` called on a leaf node"),
            Node::Intermediate { box right, .. } => right,
        }
    }

    /// Assume `self` is an intermediate node and return a reference to the free range.
    fn unchecked_free_ref(&self) -> &Range<T> {
        match *self {
            Node::Leaf { .. } => panic!("`unchecked_free_ref` called on a leaf node"),
            Node::Intermediate { ref free, .. } => free,
        }
    }

    /// Assume `self` is an intermediate node and return a mutable reference to the free range.
    fn unchecked_free_mut(&mut self) -> &mut Range<T> {
        match *self {
            Node::Leaf { .. } => panic!("`unchecked_free_mut` called on a leaf node"),
            Node::Intermediate { ref mut free, .. } => free,
        }
    }
}

/// Inserts a leaf node with given start, end and data in place of the right node of some other
/// node `x`. The original right node of `x` becomes the right node of the right node of `x` and
/// the new node becomes the left node of the right node of `x`. The free range of `x` is also
/// passed and updated.
fn unchecked_insert<'a, T, D>(start: T, end: T, data: &'a D, right: &mut Box<Node<'a, T, D>>, free: &mut Range<T>)
    where T: Ord + Copy + Debug,
          D: Debug
{
    assert!(free.start <= start);
    assert!(end <= free.end);

    let new_node = Node::Leaf {
        start: start,
        end: end,
        data: data,
    };

    take_mut::take(right, |right_value| {
        Box::new(Node::Intermediate {
            left: Box::new(new_node),
            right: right_value,
            free: end..free.end,
        })
    });

    *free = free.start..start;
}


#[derive(Debug)]
pub struct Entry<'a, T, D: 'a> {
    pub start: T,
    pub end: T,
    pub data: &'a D,
}


#[derive(Debug)]
pub struct Iter<'b, 'a: 'b, T: 'b, D: 'a> {
    path: Vec<&'b Node<'a, T, D>>,
}


impl<'b, 'a, T, D> Iterator for Iter<'b, 'a, T, D>
    where T: Copy
{
    type Item = Entry<'a, T, D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.path
            .pop()
            .and_then(|mut current| {
                while let Node::Intermediate {
                              ref left,
                              ref right,
                              ..
                          } = *current {
                    self.path.push(right);
                    current = left;
                }
                if let Node::Leaf { start, end, data } = *current {
                    Some(Entry {
                             start: start,
                             end: end,
                             data: data,
                         })
                } else {
                    None
                }
            })
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::ops::Range;

    #[test]
    fn test_schedule_exact() {
        let data = generate_data(10);

        let mut tree = ScheduleTree::new();

        // 5..9
        let scheduled = tree.schedule_exact(5, 4, &data[0]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..9));
        assert_matches!(tree.root, Some(Node::Leaf { start: 5, end: 9, .. }));

        //   free:9..13
        //    /        \
        // 5..9       13..18
        let scheduled = tree.schedule_exact(13, 5, &data[1]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 9, end: 13 },
            right: box Node::Leaf { start: 13, end: 18, .. },
        .. }));

        //   free:9..10
        //    /        \
        // 5..9      free:12..13
        //             /     \
        //          10..12  13..18
        let scheduled = tree.schedule_exact(10, 2, &data[2]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 9, end: 10 },
            right: box Node::Intermediate {
                free: Range { start: 12, end: 13 },
                left: box Node::Leaf { start: 10, end: 12, .. },
            .. },
        .. }));

        let scheduled = tree.schedule_exact(14, 2, &data[3]);
        assert!(!scheduled);

        let scheduled = tree.schedule_exact(12, 0, &data[4]);
        assert!(!scheduled);

        let scheduled = tree.schedule_exact(9, 2, &data[5]);
        assert!(!scheduled);

        //     free:9..9
        //    /         \
        // 5..9      free:10..10
        //            /       \
        //         9..10   free:12..13
        //                   /     \
        //               10..12   13..18
        let scheduled = tree.schedule_exact(9, 1, &data[6]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 9, end: 9 },
            left: box Node::Leaf { start: 5, end: 9, .. },
            right: box Node::Intermediate {
                free: Range { start: 10, end: 10 },
                left: box Node::Leaf { start: 9, end: 10, .. },
                right: box Node::Intermediate {
                    free: Range { start: 12, end: 13 },
                    left: box Node::Leaf { start: 10, end: 12, .. },
                    right: box Node::Leaf { start: 13, end: 18, .. },
                },
            },
        }));
    }

    #[test]
    fn test_schedule_close_before() {
        let data = generate_data(10);

        let mut tree = ScheduleTree::new();

        // 13..18
        let scheduled = tree.schedule_close_before(18, 5, None, &data[0]);
        assert!(scheduled);
        assert!(tree.scope == Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_close_before(10, 5, None, &data[1]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        let scheduled = tree.schedule_close_before(17, 2, Some(12), &data[2]);
        assert!(!scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        //   free:10..11
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          11..13  13..18
        let scheduled = tree.schedule_close_before(17, 2, Some(11), &data[3]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 11 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Intermediate {
                free: Range { start: 13, end: 13 },
                left: box Node::Leaf { start: 11, end: 13, .. },
                right: box Node::Leaf { start: 13, end: 18, .. }
            },
        }));

        let scheduled = tree.schedule_close_before(19, 2, Some(4), &data[4]);
        assert!(!scheduled);

        //     free:5..5
        //     /       \
        //  3..5    free:10..11
        //           /        \
        //        5..10     free:13..13
        //                    /     \
        //                 11..13  13..18
        let scheduled = tree.schedule_close_before(19, 2, Some(3), &data[5]);
        assert!(scheduled);
        assert!(tree.scope == Some(3..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 5, end: 5 },
            left: box Node::Leaf { start: 3, end: 5, .. },
            right: box Node::Intermediate {
                free: Range { start: 10, end: 11 },
                left: box Node::Leaf { start: 5, end: 10, .. },
                right: box Node::Intermediate {
                    free: Range { start: 13, end: 13 },
                    left: box Node::Leaf { start: 11, end: 13, .. },
                    right: box Node::Leaf { start: 13, end: 18, .. }
                },
            },
        }));

        //           free:18..30
        //          /           \
        //     free:5..5       25..30
        //     /       \
        //  3..5    free:10..11
        //           /        \
        //        5..10     free:13..13
        //                    /     \
        //                 11..13  13..18
        let scheduled = tree.schedule_close_before(30, 5, Some(19), &data[6]);
        assert!(scheduled);
        assert!(tree.scope == Some(3..30));

        //                free:18..21
        //              /             \
        //     free:5..5               free:24..25
        //     /       \                /        \
        //  3..5    free:10..11      21..24     25..30
        //           /        \
        //        5..10     free:13..13
        //                    /     \
        //                 11..13  13..18
        let scheduled = tree.schedule_close_before(24, 3, None, &data[7]);
        assert!(scheduled);
        assert!(tree.scope == Some(3..30));

        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 18, end: 21 },
            left: box Node::Intermediate {
                free: Range { start: 5, end: 5 },
                left: box Node::Leaf { start: 3, end: 5, .. },
                right: box Node::Intermediate {
                    free: Range { start: 10, end: 11 },
                    left: box Node::Leaf { start: 5, end: 10, .. },
                    right: box Node::Intermediate {
                        free: Range { start: 13, end: 13 },
                        left: box Node::Leaf { start: 11, end: 13, .. },
                        right: box Node::Leaf { start: 13, end: 18, .. }
                    },
                },
            },
            right: box Node::Intermediate {
                free: Range { start: 24, end: 25 },
                left: box Node::Leaf { start: 21, end: 24, .. },
                right: box Node::Leaf { start: 25, end: 30, .. },
            },
        }));
    }

    #[test]
    fn test_schedule_close_after() {
        let data = generate_data(10);

        let mut tree = ScheduleTree::new();

        // 13..18
        let scheduled = tree.schedule_close_after(13, 5, None, &data[0]);
        assert!(scheduled);
        assert!(tree.scope == Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_close_after(5, 5, Some(10), &data[1]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        let scheduled = tree.schedule_close_after(4, 2, Some(11), &data[2]);
        assert!(!scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        //   free:10..10
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          10..13  13..18
        let scheduled = tree.schedule_close_after(4, 3, Some(13), &data[3]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 10 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Intermediate {
                free: Range { start: 13, end: 13 },
                left: box Node::Leaf { start: 10, end: 13, .. },
                right: box Node::Leaf { start: 13, end: 18, .. }
            },
        }));

        let scheduled = tree.schedule_close_after(4, 2, Some(19), &data[4]);
        assert!(!scheduled);

        //         free:18..18
        //         /          \
        //   free:10..10     18..20
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          10..13  13..18
        let scheduled = tree.schedule_close_after(4, 2, Some(20), &data[5]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..20));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 18, end: 18 },
            left: box Node::Intermediate {
                free: Range { start: 10, end: 10 },
                left: box Node::Leaf { start: 5, end: 10, .. },
                right: box Node::Intermediate {
                    free: Range { start: 13, end: 13 },
                    left: box Node::Leaf { start: 10, end: 13, .. },
                    right: box Node::Leaf { start: 13, end: 18, .. }
                },
            },
            right: box Node::Leaf { start: 18, end: 20, .. },
        }));

        //                free:20..25
        //              /             \
        //         free:18..18       25..30
        //         /          \
        //   free:10..10     18..20
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          10..13  13..18
        let scheduled = tree.schedule_close_after(25, 5, None, &data[6]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..30));

        //                      free:20..21
        //                    /             \
        //         free:18..18               free:23..25
        //         /          \              /         \
        //   free:10..10     18..20      21..23       25..30
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          10..13  13..18
        let scheduled = tree.schedule_close_after(21, 2, None, &data[7]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..30));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 20, end: 21 },
            left: box Node::Intermediate {
                free: Range { start: 18, end: 18 },
                left: box Node::Intermediate {
                    free: Range { start: 10, end: 10 },
                    left: box Node::Leaf { start: 5, end: 10, .. },
                    right: box Node::Intermediate {
                        free: Range { start: 13, end: 13 },
                        left: box Node::Leaf { start: 10, end: 13, .. },
                        right: box Node::Leaf { start: 13, end: 18, .. }
                    },
                },
                right: box Node::Leaf { start: 18, end: 20, .. },
            },
            right: box Node::Intermediate {
                free: Range { start: 23, end: 25 },
                left: box Node::Leaf { start: 21, end: 23, .. },
                right: box Node::Leaf { start: 25, end: 30, .. },
            },
        }));
    }

    #[test]
    fn test_unschedule() {
        let data = generate_data(10);

        let mut tree: ScheduleTree<i8, i8> = ScheduleTree::new();

        // 5..9
        // =>
        // <empty>
        tree.schedule_exact(5, 4, &data[0]);
        let entry = tree.unschedule(&data[0]);
        assert_matches!(entry, Some(Entry { start: 5, end: 9, .. }));
        assert_matches!(tree, ScheduleTree { root: None, scope: None, .. });
        assert!(tree.data_map.is_empty());

        //   free:9..13
        //    /        \
        // 5..9       13..18
        // =>
        // 5..9
        tree.schedule_exact(5, 4, &data[0]);
        tree.schedule_exact(13, 5, &data[1]);
        let entry = tree.unschedule(&data[1]);
        assert_matches!(entry, Some(Entry { start: 13, end: 18, .. }));
        assert_eq!(tree.scope, Some(5..9));
        assert_matches!(tree.root, Some(Node::Leaf { start: 5, end: 9, .. }));

        //   free:9..13
        //    /        \
        // 5..9       13..18
        // =>
        // 13..18
        tree.schedule_exact(5, 4, &data[0]);
        tree.schedule_exact(13, 5, &data[1]);
        let entry = tree.unschedule(&data[0]);
        assert_matches!(entry, Some(Entry { start: 5, end: 9, .. }));
        assert_eq!(tree.scope, Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        // 13..18
        // =>
        //   free:9..10
        //    /        \
        // 5..9      free:12..13
        //             /     \
        //          10..12  13..18
        // =>
        // free:12..13
        //    /     \
        // 10..12  13..18
        // =>
        // 13..18
        tree.schedule_close_before(9, 4, None, &data[0]);
        tree.schedule_close_after(10, 2, None, &data[2]);

        let entry = tree.unschedule(&data[0]);
        assert_matches!(entry, Some(Entry { start: 5, end: 9, .. }));
        assert_eq!(tree.scope, Some(10..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 12, end: 13 },
            left: box Node::Leaf { start: 10, end: 12, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        let entry = tree.unschedule(&data[2]);
        assert_matches!(entry, Some(Entry { start: 10, end: 12, .. }));
        assert_eq!(tree.scope, Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        // 13..18
        // =>
        //   free:9..10
        //    /        \
        // 5..9      free:12..13
        //             /     \
        //          10..12  13..18
        // =>
        //   free:9..13
        //    /     \
        // 5..9    13..18
        // =>
        // 13..18
        // =>
        // <empty>
        assert_eq!(tree.scope, Some(13..18));
        tree.schedule_close_after(10, 2, None, &data[0]);
        assert_eq!(tree.scope, Some(10..18));
        tree.schedule_close_before(9, 4, None, &data[2]);
        assert_eq!(tree.scope, Some(5..18));

        let entry = tree.unschedule(&data[0]);
        assert_matches!(entry, Some(Entry { start: 10, end: 12, .. }));
        assert_eq!(tree.scope, Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 9, end: 13 },
            left: box Node::Leaf { start: 5, end: 9, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        let entry = tree.unschedule(&data[2]);
        assert_matches!(entry, Some(Entry { start: 5, end: 9, .. }));
        assert_eq!(tree.scope, Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        let entry = tree.unschedule(&data[1]);
        assert_matches!(entry, Some(Entry { start: 13, end: 18, .. }));
        assert_matches!(tree, ScheduleTree { root: None, scope: None, .. });
        assert!(tree.data_map.is_empty());
    }

    fn generate_data(n: i8) -> Vec<i8> {
        (0..n).collect()
    }
}
