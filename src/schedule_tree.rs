use std::fmt::Debug;
use std::ops::{Add, Range, Sub};

use take_mut;


#[derive(Debug)]
pub struct ScheduleTree<'a, T, D: 'a> {
    root: Option<Node<'a, T, D>>,
    scope: Option<Range<T>>,
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
          D: Debug
{
    /// Returns an empty schedule tree.
    pub fn new() -> Self {
        ScheduleTree {
            root: None,
            scope: None,
        }
    }

    /// Returns a chronological iterator of the schedule tree.
    pub fn iter<'b>(&'b self) -> Iter<'b, 'a, T, D> {
        Iter { path: self.root.iter().collect() }
    }

    /// Tries to schedule `data` at the exact `start` with the given `duration`.
    ///
    /// Returns whether the scheduling succeeded.
    #[allow(dead_code)]
    pub fn schedule_exact<W>(&mut self, start: T, duration: W, data: &'a D) -> bool
        where T: Add<W, Output = T>
    {
        let end = start + duration;
        if self.try_schedule_trivial_cases(start, end, data) {
            return true
        }

        self.root.as_mut().unwrap().insert(start, end, data)
    }

    /// Tries to schedule `data` as close as possible before `end` with the given `duration`. It
    /// must be scheduled after `min_start` when given.
    ///
    /// Returns whether the scheduling succeeded.
    pub fn schedule_close_before<W>(&mut self, end: T, duration: W, min_start: Option<T>, data: &'a D) -> bool
        where T: Add<W, Output = T> + Sub<W, Output = T>,
              W: Copy
    {
        assert!(min_start.map_or(true, |min_start| min_start + duration <= end));

        let optimal_start = end - duration;
        if self.try_schedule_trivial_cases(optimal_start, end, data) {
            return true
        }

        if self.root.as_mut().unwrap().insert_before(end, duration, min_start, data) {
            return true
        }

        // As last resort, try to schedule before current scope if min_start allows
        let scope = self.scope.as_ref().cloned().unwrap();
        if min_start.map_or(true, |min_start| min_start <= scope.start - duration) {
            // Schedule on [scope.start - duration, scope.start]
            let new_node = Node::Leaf {
                start: scope.start - duration,
                end: scope.start,
                data: data,
            };
            self.root = Some(Node::Intermediate {
                                 left: Box::new(new_node),
                                 right: Box::new(self.root.take().unwrap()),
                                 free: scope.start..scope.start,
                             });
            self.scope = Some((scope.start - duration)..scope.end);
            return true
        }

        false
    }

    /// Tries to schedule `data` as close as possible after `start` with the given `duration`. It
    /// must be scheduled before `max_end` when given.
    ///
    /// Returns whether the scheduling succeeded.
    pub fn schedule_close_after<W>(&mut self, start: T, duration: W, max_end: Option<T>, data: &'a D) -> bool
        where T: Add<W, Output = T> + Sub<W, Output = T>,
              W: Copy + Debug
    {
        assert!(max_end.map_or(true, |max_end| start + duration <= max_end));

        let optimal_end = start + duration;
        if self.try_schedule_trivial_cases(start, optimal_end, data) {
            return true
        }

        if self.root.as_mut().unwrap().insert_after(start, duration, max_end, data) {
            return true
        }

        // As last resort, try to schedule after current scope if max_end allows
        let scope = self.scope.as_ref().cloned().unwrap();
        if max_end.map_or(true, |max_end| scope.end + duration <= max_end) {
            // Schedule on [scope.end, scope.end + duration]
            let new_node = Node::Leaf {
                start: scope.end,
                end: scope.end + duration,
                data: data,
            };
            self.root = Some(Node::Intermediate {
                                 left: Box::new(self.root.take().unwrap()),
                                 right: Box::new(new_node),
                                 free: scope.end..scope.end,
                             });
            self.scope = Some(scope.start..(scope.end + duration));
            return true
        }

        false
    }

    /// Common scheduling cases between all scheduling strategies. It handles the cases where
    /// (a) the schedule tree is empty;
    /// (b) the most optimal start and end fall completely before the left-most child in the tree
    /// (c) the most optimal start and end fall completely after the right-most child in the tree
    ///
    /// Returns whether the scheduling succeeded.
    fn try_schedule_trivial_cases(&mut self, start: T, end: T, data: &'a D) -> bool {
        let new_node = Node::Leaf {
            start: start,
            end: end,
            data: data,
        };

        if self.root.is_none() {
            self.root = Some(new_node);
            self.scope = Some(start..end);
            return true
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
            return true
        } else if scope.end <= start {
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                                 left: Box::new(root),
                                 right: Box::new(new_node),
                                 free: scope.end..start,
                             });
            self.scope = Some(scope.start..end);
            return true
        }

        false
    }
}


impl<'a, T, D> Node<'a, T, D>
    where T: Copy + Ord + Debug,
          D: Debug
{
    /// Tries to insert a node with given `start`, `end` and `data` as a descendant of this node.
    fn insert(&mut self, start: T, end: T, data: &'a D) -> bool {
        match *self {
            Node::Leaf { .. } => false,
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
                    true
                } else {
                    // Overlap between [start, end] and self.free
                    false
                }
            }
        }
    }

    /// Tries to insert a node with the given `data` and `duration` as a descendant of this node.
    /// It must be scheduled as close before `end` as possible, but it cannot be scheduled sooner
    /// than `min_start`, when given.
    fn insert_before<W>(&mut self, end: T, duration: W, min_start: Option<T>, data: &'a D) -> bool
        where T: Sub<W, Output = T>,
              W: Copy
    {
        match *self {
            Node::Leaf { .. } => false,
            Node::Intermediate {
                ref mut left,
                ref mut right,
                ref mut free,
            } => {
                // If the end is inside the right child, try that first
                if free.end < end {
                    if right.insert_before(end, duration, min_start, data) {
                        return true
                    }
                }
                // Second, try to insert it in the free range of the current node
                if free.start <= free.end - duration {
                    if min_start.map_or(true, |min_start| min_start <= free.end - duration) {
                        unchecked_insert(free.end - duration, free.end, data, right, free);
                        return true
                    }
                }
                // If min_start is contained in free, don't bother checking the left child
                if min_start.map_or(true, |min_start| free.start <= min_start) {
                    return false
                }
                // Last, try to insert it in the left child
                left.insert_before(end, duration, min_start, data)
            }
        }
    }

    /// Tries to insert a node with the given `data` and `duration` as a descendant of this node.
    /// It must be scheduled as close after `start` as possible, but it cannot be scheduled later
    /// than `max_end`, when given.
    fn insert_after<W>(&mut self, start: T, duration: W, max_end: Option<T>, data: &'a D) -> bool
        where T: Add<W, Output = T>,
              W: Copy
    {
        match *self {
            Node::Leaf { .. } => false,
            Node::Intermediate {
                ref mut left,
                ref mut right,
                ref mut free,
            } => {
                // If the start is inside the left child, try that first
                if start < free.start {
                    if left.insert_after(start, duration, max_end, data) {
                        return true
                    }
                }
                // Second, try to insert it in the free range of the current node
                if free.start + duration <= free.end {
                    if max_end.map_or(true, |max_end| free.start + duration <= max_end) {
                        // TODO insert right?
                        unchecked_insert(free.start, free.start + duration, data, right, free);
                        return true
                    }
                }
                // If max_end is contained in free, don't bother checking the right child
                if max_end.map_or(true, |max_end| max_end <= free.end) {
                    return false
                }
                // Last, try to insert it in the right child
                right.insert_after(start, duration, max_end, data)
            }
        }
    }
}

/// Inserts a leaf node with given start, end and data in place of the right node of some other
/// node `x`. The original right node of `x` becomes the right node of the right node of `x` and
/// the new node becomes the left node of the right node of `x`. The free range of `x` is also
/// passed and updated.
fn unchecked_insert<'a, T, D>(start: T, end: T, data: &'a D, right: &mut Box<Node<'a, T, D>>, free: &mut Range<T>)
    where T: Ord + Copy
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

    const DATA: &i8 = &9;

    #[test]
    fn test_schedule_exact() {
        let mut tree = ScheduleTree::new();

        // 5..9
        let scheduled = tree.schedule_exact(5, 4, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..9));
        assert_matches!(tree.root, Some(Node::Leaf { start: 5, end: 9, .. }));

        //   free:9..13
        //    /        \
        // 5..9       13..18
        let scheduled = tree.schedule_exact(13, 5, DATA);
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
        let scheduled = tree.schedule_exact(10, 2, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 9, end: 10 },
            right: box Node::Intermediate {
                free: Range { start: 12, end: 13 },
                left: box Node::Leaf { start: 10, end: 12, .. },
            .. },
        .. }));

        let scheduled = tree.schedule_exact(14, 2, DATA);
        assert!(!scheduled);

        let scheduled = tree.schedule_exact(12, 0, DATA);
        assert!(!scheduled);

        let scheduled = tree.schedule_exact(9, 2, DATA);
        assert!(!scheduled);

        //     free:9..9
        //    /         \
        // 5..9      free:10..10
        //            /       \
        //         9..10   free:12..13
        //                   /     \
        //               10..12   13..18
        let scheduled = tree.schedule_exact(9, 1, DATA);
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
        let mut tree = ScheduleTree::new();

        // 13..18
        let scheduled = tree.schedule_close_before(18, 5, None, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_close_before(10, 5, None, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        let scheduled = tree.schedule_close_before(17, 2, Some(12), DATA);
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
        let scheduled = tree.schedule_close_before(17, 2, Some(11), DATA);
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

        let scheduled = tree.schedule_close_before(19, 2, Some(4), DATA);
        assert!(!scheduled);

        //     free:5..5
        //     /       \
        //  3..5    free:10..11
        //           /        \
        //        5..10     free:13..13
        //                    /     \
        //                 11..13  13..18
        let scheduled = tree.schedule_close_before(19, 2, Some(3), DATA);
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
    }

    #[test]
    fn test_schedule_close_after() {
        let mut tree = ScheduleTree::new();

        // 13..18
        let scheduled = tree.schedule_close_after(13, 5, None, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(13..18));
        assert_matches!(tree.root, Some(Node::Leaf { start: 13, end: 18, .. }));

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_close_after(5, 5, Some(10), DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            left: box Node::Leaf { start: 5, end: 10, .. },
            right: box Node::Leaf { start: 13, end: 18, .. },
        }));

        let scheduled = tree.schedule_close_after(4, 2, Some(11), DATA);
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
        let scheduled = tree.schedule_close_after(4, 3, Some(13), DATA);
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

        let scheduled = tree.schedule_close_after(4, 2, Some(19), DATA);
        assert!(!scheduled);

        //         free:18..18
        //         /          \
        //   free:10..10     18..20
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          10..13  13..18
        let scheduled = tree.schedule_close_after(4, 2, Some(20), DATA);
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
    }
}
