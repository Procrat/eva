use std::cmp::{max, min};
use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::ops::{Add, Range, Sub};
use std::rc::Rc;

use crate::util::WithSideEffects;

macro_rules! return_on_some {
    ($e:expr) => {
        if let Some(value) = $e {
            return Some(value);
        }
    };
}

#[derive(Debug, Default)]
pub struct ScheduleTree<T, D: Eq + Hash> {
    root: Option<Node<T, D>>,
    scope: Option<Range<T>>,
    data_map: HashMap<Rc<D>, T>,
}

#[derive(Debug, PartialEq)]
pub enum Node<T, D> {
    Leaf {
        start: T,
        end: T,
        data: Rc<D>,
    },
    Intermediate {
        free: Range<T>,
        left: Box<Node<T, D>>,
        right: Box<Node<T, D>>,
    },
}

impl<T, D> ScheduleTree<T, D>
where
    T: Copy + Ord + Debug,
    D: Debug + Eq + Hash,
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
    pub fn iter(&self) -> Iter<T, D> {
        Iter {
            path: self.root.iter().collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Tries to schedule `data` at the exact `start` with the given `duration`.
    ///
    /// Returns whether the scheduling succeeded.
    #[allow(dead_code)]
    pub fn schedule_exact<W>(&mut self, start: T, duration: W, data: D) -> bool
    where
        T: Add<W, Output = T>,
    {
        let data = Rc::new(data);
        self.schedule_exact_(start, duration, Rc::clone(&data))
            .with_side_effects(|start| self.update_map(start, data))
            .is_some()
    }

    /// See `schedule_exact` for details.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn schedule_exact_<W>(&mut self, start: T, duration: W, data: Rc<D>) -> Option<T>
    where
        T: Add<W, Output = T>,
    {
        let end = start + duration;
        return_on_some!(self.try_schedule_trivial_cases(start, end, Rc::clone(&data)));

        self.root
            .as_mut()
            .expect("Internal error: root could not be taken as mut ref")
            .insert(start, end, data)
    }

    /// Tries to schedule `data` as close as possible before `end` with the given `duration`. It
    /// must be scheduled after `min_start` when given.
    ///
    /// Returns whether the scheduling succeeded.
    pub fn schedule_close_before<W>(
        &mut self,
        end: T,
        duration: W,
        min_start: Option<T>,
        data: D,
    ) -> bool
    where
        T: Add<W, Output = T> + Sub<W, Output = T>,
        W: Copy + Debug,
    {
        let data = Rc::new(data);
        self.schedule_close_before_(end, duration, min_start, Rc::clone(&data))
            .with_side_effects(|start| self.update_map(start, data))
            .is_some()
    }

    /// See `schedule_close_before` for details.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn schedule_close_before_<W>(
        &mut self,
        end: T,
        duration: W,
        min_start: Option<T>,
        data: Rc<D>,
    ) -> Option<T>
    where
        T: Add<W, Output = T> + Sub<W, Output = T>,
        W: Copy + Debug,
    {
        assert!(min_start.map_or(true, |min_start| min_start + duration <= end));

        let optimal_start = end - duration;
        return_on_some!(self.try_schedule_trivial_cases(optimal_start, end, Rc::clone(&data)));

        return_on_some!(self
            .root
            .as_mut()
            .expect("Internal error: root could not be taken as mut ref")
            .insert_before(end, duration, min_start, Rc::clone(&data)));

        // As last resort, try to schedule before current scope if min_start allows
        let scope = self
            .scope
            .as_mut()
            .expect("Internal error: scope could not be taken as ref");
        if min_start.map_or(true, |min_start| min_start <= scope.start - duration) {
            // Schedule on [scope.start - duration, scope.start]
            let start = scope.start - duration;
            let end = scope.start;
            let new_node = Node::Leaf { start, end, data };
            self.root = Some(Node::Intermediate {
                left: Box::new(new_node),
                right: Box::new(
                    self.root
                        .take()
                        .expect("Internal error: root could not be taken"),
                ),
                free: scope.start..scope.start,
            });
            self.scope = Some(start..scope.end);
            return Some(start);
        }

        None
    }

    /// Tries to schedule `data` as close as possible after `start` with the given `duration`. It
    /// must be scheduled before `max_end` when given.
    ///
    /// Returns whether the scheduling succeeded.
    pub fn schedule_close_after<W>(
        &mut self,
        start: T,
        duration: W,
        max_end: Option<T>,
        data: D,
    ) -> bool
    where
        T: Add<W, Output = T> + Sub<W, Output = T>,
        W: Copy + Debug,
    {
        let data = Rc::new(data);
        self.schedule_close_after_(start, duration, max_end, Rc::clone(&data))
            .with_side_effects(|start| self.update_map(start, data))
            .is_some()
    }

    /// See `schedule_close_after` for details.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn schedule_close_after_<W>(
        &mut self,
        start: T,
        duration: W,
        max_end: Option<T>,
        data: Rc<D>,
    ) -> Option<T>
    where
        T: Add<W, Output = T> + Sub<W, Output = T>,
        W: Copy + Debug,
    {
        assert!(max_end.map_or(true, |max_end| start + duration <= max_end));

        let optimal_end = start + duration;
        return_on_some!(self.try_schedule_trivial_cases(start, optimal_end, Rc::clone(&data)));

        return_on_some!(self
            .root
            .as_mut()
            .expect("Internal error: root could not be taken as mut ref")
            .insert_after(start, duration, max_end, Rc::clone(&data)));

        // As last resort, try to schedule after current scope if max_end allows
        let scope = self
            .scope
            .as_mut()
            .expect("Internal error: scope could not be taken as ref");
        if max_end.map_or(true, |max_end| scope.end + duration <= max_end) {
            // Schedule on [scope.end, scope.end + duration]
            let start = scope.end;
            let end = scope.end + duration;
            let new_node = Node::Leaf { start, end, data };
            self.root = Some(Node::Intermediate {
                left: Box::new(
                    self.root
                        .take()
                        .expect("Internal error: root could not be taken"),
                ),
                right: Box::new(new_node),
                free: scope.end..scope.end,
            });
            self.scope = Some(scope.start..end);
            return Some(start);
        }

        None
    }

    /// Common scheduling cases between all scheduling strategies. It handles the cases where
    /// (a) the schedule tree is empty;
    /// (b) the most optimal start and end fall completely before the left-most child in the tree
    /// (c) the most optimal start and end fall completely after the right-most child in the tree
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn try_schedule_trivial_cases(&mut self, start: T, end: T, data: Rc<D>) -> Option<T> {
        let new_node = Node::Leaf { start, end, data };

        match (self.root.take(), self.scope.take()) {
            (None, None) => {
                self.root = Some(new_node);
                self.scope = Some(start..end);
                Some(start)
            }
            (Some(root), Some(scope)) => {
                if end <= scope.start {
                    self.root = Some(Node::Intermediate {
                        left: Box::new(new_node),
                        right: Box::new(root),
                        free: end..scope.start,
                    });
                    self.scope = Some(start..scope.end);
                    Some(start)
                } else if scope.end <= start {
                    self.root = Some(Node::Intermediate {
                        left: Box::new(root),
                        right: Box::new(new_node),
                        free: scope.end..start,
                    });
                    self.scope = Some(scope.start..end);
                    Some(start)
                } else {
                    self.root = Some(root);
                    self.scope = Some(scope);
                    None
                }
            }
            _ => unreachable!(),
        }
    }

    /// Removes the given data from the schedule tree.
    ///
    /// Returns the related entry from the tree if the tree contained it, otherwise None.
    pub fn unschedule<'a>(&mut self, data: &'a D) -> Option<Entry<T, D>> {
        let when = self.remove_from_map(data);
        match (self.root.take(), when) {
            (Some(mut root), Some(when)) => match root {
                Node::Leaf { start, end, data } => {
                    self.root = None;
                    self.scope = None;
                    Some(Entry {
                        start,
                        end,
                        data: Rc::try_unwrap(data).expect("Internal error: rc was not 1"),
                    })
                }
                Node::Intermediate { .. } => {
                    let entry = root.unschedule(when, data).map(|(entry, scope)| {
                        self.scope = Some(scope);
                        Entry {
                            start: entry.start,
                            end: entry.end,
                            data: Rc::try_unwrap(entry.data).expect("Internal error: rc was not 1"),
                        }
                    });
                    self.root = Some(root);
                    entry
                }
            },
            _ => None,
        }
    }

    pub fn when_scheduled<'a>(&self, data: &'a D) -> Option<&T> {
        self.data_map.get(data)
    }

    fn remove_from_map<'a>(&mut self, data: &'a D) -> Option<T> {
        self.data_map.remove(data)
    }

    fn update_map(&mut self, start: T, data: Rc<D>) {
        let old_value = self.data_map.insert(data, start);
        if old_value.is_some() {
            panic!("Internal error: same data is being entered twice")
        }
    }
}

impl<T, D> Node<T, D>
where
    T: Copy + Ord + Debug,
    D: Debug,
{
    /// Tries to insert a node with given `start`, `end` and `data` as a descendant of this node.
    ///
    /// Returns the start of the scheduling if it succeeded, otherwise None
    fn insert(&mut self, start: T, end: T, data: Rc<D>) -> Option<T> {
        match self {
            Node::Leaf { .. } => None,
            Node::Intermediate { left, right, free } => {
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
    fn insert_before<W>(
        &mut self,
        end: T,
        duration: W,
        min_start: Option<T>,
        data: Rc<D>,
    ) -> Option<T>
    where
        T: Sub<W, Output = T>,
        W: Copy + Debug,
    {
        match self {
            Node::Leaf { .. } => None,
            Node::Intermediate { left, right, free } => {
                // If the end is inside the right child, try that first
                if free.end < end {
                    return_on_some!(right.insert_before(end, duration, min_start, Rc::clone(&data)))
                }
                // Second, try to insert it in the free range of the current node
                let end = min(end, free.end);
                if free.start <= end - duration
                    && min_start.map_or(true, |min_start| min_start <= end - duration)
                {
                    unchecked_insert(end - duration, end, Rc::clone(&data), right, free);
                    return Some(end - duration);
                }

                // If min_start is contained in free, don't bother checking the left child
                if min_start.map_or(true, |min_start| free.start <= min_start) {
                    return None;
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
    fn insert_after<W>(
        &mut self,
        start: T,
        duration: W,
        max_end: Option<T>,
        data: Rc<D>,
    ) -> Option<T>
    where
        T: Ord + Add<W, Output = T>,
        W: Copy + Debug,
    {
        match self {
            Node::Leaf { .. } => None,
            Node::Intermediate { left, right, free } => {
                // If the start is inside the left child, try that first
                if start < free.start {
                    return_on_some!(left.insert_after(start, duration, max_end, Rc::clone(&data)))
                }
                // Second, try to insert it in the free range of the current node
                let start = max(start, free.start);
                if start + duration <= free.end
                    && max_end.map_or(true, |max_end| start + duration <= max_end)
                {
                    unchecked_insert(start, start + duration, data, right, free);
                    return Some(start);
                }
                // If max_end is contained in free, don't bother checking the right child
                if max_end.map_or(true, |max_end| max_end <= free.end) {
                    return None;
                }
                // Last, try to insert it in the right child
                right.insert_after(start, duration, max_end, data)
            }
        }
    }

    /// Tries to unschedule the given `data` which is scheduled at the given `start`.
    ///
    /// Returns None if that combination wasn't found, otherwise a tuple of an entry representing
    /// the unscheduled item and the new scope of this node.
    fn unschedule<'a>(&mut self, start: T, data: &'a D) -> Option<(Entry<T, Rc<D>>, Range<T>)>
    where
        D: PartialEq,
    {
        match self {
            Node::Leaf { .. } => panic!("Internal error: `unschedule` called on a leaf node"),
            Node::Intermediate { left, right, free } => {
                if start < free.start {
                    match left {
                        box Node::Leaf {
                            start: left_start,
                            data: left_data,
                            ..
                        } => {
                            if start == *left_start && *data == **left_data {
                                let mut entry = None;
                                take_mut::take(self, |self_| match self_ {
                                    Node::Intermediate {
                                        left: box Node::Leaf { start, end, data },
                                        right,
                                        ..
                                    } => {
                                        entry = Some(Entry { start, end, data });
                                        *right
                                    }
                                    _ => unreachable!(),
                                });
                                entry.map(|entry| (entry, self.find_scope()))
                            } else {
                                None
                            }
                        }
                        box Node::Intermediate { .. } => {
                            left.unschedule(start, data).map(|(entry, scope)| {
                                free.start = scope.end;
                                (entry, scope.start..right.find_scope().end)
                            })
                        }
                    }
                } else if free.end <= start {
                    match right {
                        box Node::Leaf {
                            start: right_start,
                            data: right_data,
                            ..
                        } => {
                            if start == *right_start && *data == **right_data {
                                let mut entry = None;
                                take_mut::take(self, |self_| match self_ {
                                    Node::Intermediate {
                                        left,
                                        right: box Node::Leaf { start, end, data },
                                        ..
                                    } => {
                                        entry = Some(Entry { start, end, data });
                                        *left
                                    }
                                    _ => unreachable!(),
                                });
                                entry.map(|entry| (entry, self.find_scope()))
                            } else {
                                None
                            }
                        }
                        box Node::Intermediate { .. } => {
                            right.unschedule(start, data).map(|(entry, scope)| {
                                free.end = scope.start;
                                (entry, left.find_scope().start..scope.end)
                            })
                        }
                    }
                } else {
                    None
                }
            }
        }
    }

    /// Calculates the scope of all descendants of this node.
    fn find_scope(&self) -> Range<T> {
        match self {
            Node::Leaf { start, end, .. } => *start..*end,
            Node::Intermediate { left, right, .. } => {
                let start = left.find_scope().start;
                let end = right.find_scope().end;
                start..end
            }
        }
    }
}

/// Inserts a leaf node with given start, end and data in place of the right node of some other
/// node `x`. The original right node of `x` becomes the right node of the right node of `x` and
/// the new node becomes the left node of the right node of `x`. The free range of `x` is also
/// passed and updated.
fn unchecked_insert<T, D>(
    start: T,
    end: T,
    data: Rc<D>,
    right: &mut Node<T, D>,
    free: &mut Range<T>,
) where
    T: Ord + Copy + Debug,
    D: Debug,
{
    assert!(free.start <= start);
    assert!(end <= free.end);

    let new_node = Node::Leaf { start, end, data };

    take_mut::take(right, |right_value| Node::Intermediate {
        left: Box::new(new_node),
        right: Box::new(right_value),
        free: end..free.end,
    });

    *free = free.start..start;
}

#[derive(Debug)]
pub struct Entry<T, D> {
    pub start: T,
    pub end: T,
    pub data: D,
}

#[derive(Debug)]
pub struct Iter<'a, T: 'a, D: 'a> {
    path: Vec<&'a Node<T, D>>,
}

#[derive(Debug)]
pub struct IntoIter<T, D: Eq + Hash> {
    path: Vec<Node<T, D>>,
    data_map: HashMap<Rc<D>, T>,
}

impl<'a, T, D> IntoIterator for &'a ScheduleTree<T, D>
where
    T: Copy + Debug + Ord,
    D: Debug + Eq + Hash,
{
    type IntoIter = Iter<'a, T, D>;
    type Item = Entry<T, &'a D>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<T, D> IntoIterator for ScheduleTree<T, D>
where
    D: Debug + Eq + Hash,
{
    type IntoIter = IntoIter<T, D>;
    type Item = Entry<T, D>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            path: self.root.into_iter().collect(),
            data_map: self.data_map,
        }
    }
}

impl<'a, T, D> Iterator for Iter<'a, T, D>
where
    T: Copy,
{
    type Item = Entry<T, &'a D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.path.pop().and_then(|mut current: &'a Node<T, D>| {
            while let Node::Intermediate { left, right, .. } = current {
                self.path.push(right);
                current = left;
            }
            if let Node::Leaf { start, end, data } = current {
                Some(Entry {
                    start: *start,
                    end: *end,
                    data: data.as_ref(),
                })
            } else {
                None
            }
        })
    }
}

impl<T, D> Iterator for IntoIter<T, D>
where
    D: Debug + Eq + Hash,
{
    type Item = Entry<T, D>;

    fn next(&mut self) -> Option<Self::Item> {
        self.path.pop().and_then(|mut current: Node<T, D>| {
            while let Node::Intermediate { left, right, .. } = current {
                self.path.push(*right);
                current = *left;
            }
            if let Node::Leaf { start, end, data } = current {
                self.data_map.remove(&data);
                let data = Rc::try_unwrap(data).expect("Internal error: rc was more than 1");
                Some(Entry { start, end, data })
            } else {
                None
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_schedule_exact() {
        let data = generate_data(10);

        let mut tree = ScheduleTree::new();

        // 5..9
        let scheduled = tree.schedule_exact(5, 4, &data[0]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..9));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 5,
                end: 9,
                ..
            })
        );

        //   free:9..13
        //    /        \
        // 5..9       13..18
        let scheduled = tree.schedule_exact(13, 5, &data[1]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 9, end: 13 },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
                ..
            })
        );

        //   free:9..10
        //    /        \
        // 5..9      free:12..13
        //             /     \
        //          10..12  13..18
        let scheduled = tree.schedule_exact(10, 2, &data[2]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 9, end: 10 },
                right: box Node::Intermediate {
                    free: Range { start: 12, end: 13 },
                    left: box Node::Leaf {
                        start: 10,
                        end: 12,
                        ..
                    },
                    ..
                },
                ..
            })
        );

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
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 9, end: 9 },
                left: box Node::Leaf {
                    start: 5,
                    end: 9,
                    ..
                },
                right: box Node::Intermediate {
                    free: Range { start: 10, end: 10 },
                    left: box Node::Leaf {
                        start: 9,
                        end: 10,
                        ..
                    },
                    right: box Node::Intermediate {
                        free: Range { start: 12, end: 13 },
                        left: box Node::Leaf {
                            start: 10,
                            end: 12,
                            ..
                        },
                        right: box Node::Leaf {
                            start: 13,
                            end: 18,
                            ..
                        },
                    },
                },
            })
        );
    }

    #[test]
    fn test_schedule_close_before() {
        let data = generate_data(10);

        let mut tree = ScheduleTree::new();

        // 13..18
        let scheduled = tree.schedule_close_before(18, 5, None, &data[0]);
        assert!(scheduled);
        assert!(tree.scope == Some(13..18));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 13,
                end: 18,
                ..
            })
        );

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_close_before(10, 5, None, &data[1]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 10, end: 13 },
                left: box Node::Leaf {
                    start: 5,
                    end: 10,
                    ..
                },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
            })
        );

        let scheduled = tree.schedule_close_before(17, 2, Some(12), &data[2]);
        assert!(!scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 10, end: 13 },
                left: box Node::Leaf {
                    start: 5,
                    end: 10,
                    ..
                },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
            })
        );

        //   free:10..11
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          11..13  13..18
        let scheduled = tree.schedule_close_before(17, 2, Some(11), &data[3]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 10, end: 11 },
                left: box Node::Leaf {
                    start: 5,
                    end: 10,
                    ..
                },
                right: box Node::Intermediate {
                    free: Range { start: 13, end: 13 },
                    left: box Node::Leaf {
                        start: 11,
                        end: 13,
                        ..
                    },
                    right: box Node::Leaf {
                        start: 13,
                        end: 18,
                        ..
                    },
                },
            })
        );

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
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 5, end: 5 },
                left: box Node::Leaf {
                    start: 3,
                    end: 5,
                    ..
                },
                right: box Node::Intermediate {
                    free: Range { start: 10, end: 11 },
                    left: box Node::Leaf {
                        start: 5,
                        end: 10,
                        ..
                    },
                    right: box Node::Intermediate {
                        free: Range { start: 13, end: 13 },
                        left: box Node::Leaf {
                            start: 11,
                            end: 13,
                            ..
                        },
                        right: box Node::Leaf {
                            start: 13,
                            end: 18,
                            ..
                        },
                    },
                },
            })
        );

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

        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 18, end: 21 },
                left: box Node::Intermediate {
                    free: Range { start: 5, end: 5 },
                    left: box Node::Leaf {
                        start: 3,
                        end: 5,
                        ..
                    },
                    right: box Node::Intermediate {
                        free: Range { start: 10, end: 11 },
                        left: box Node::Leaf {
                            start: 5,
                            end: 10,
                            ..
                        },
                        right: box Node::Intermediate {
                            free: Range { start: 13, end: 13 },
                            left: box Node::Leaf {
                                start: 11,
                                end: 13,
                                ..
                            },
                            right: box Node::Leaf {
                                start: 13,
                                end: 18,
                                ..
                            },
                        },
                    },
                },
                right: box Node::Intermediate {
                    free: Range { start: 24, end: 25 },
                    left: box Node::Leaf {
                        start: 21,
                        end: 24,
                        ..
                    },
                    right: box Node::Leaf {
                        start: 25,
                        end: 30,
                        ..
                    },
                },
            })
        );
    }

    #[test]
    fn test_schedule_close_after() {
        let data = generate_data(10);

        let mut tree = ScheduleTree::new();

        // 13..18
        let scheduled = tree.schedule_close_after(13, 5, None, &data[0]);
        assert!(scheduled);
        assert!(tree.scope == Some(13..18));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 13,
                end: 18,
                ..
            })
        );

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_close_after(5, 5, Some(10), &data[1]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 10, end: 13 },
                left: box Node::Leaf {
                    start: 5,
                    end: 10,
                    ..
                },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
            })
        );

        let scheduled = tree.schedule_close_after(4, 2, Some(11), &data[2]);
        assert!(!scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 10, end: 13 },
                left: box Node::Leaf {
                    start: 5,
                    end: 10,
                    ..
                },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
            })
        );

        //   free:10..10
        //    /        \
        // 5..10     free:13..13
        //             /     \
        //          10..13  13..18
        let scheduled = tree.schedule_close_after(4, 3, Some(13), &data[3]);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 10, end: 10 },
                left: box Node::Leaf {
                    start: 5,
                    end: 10,
                    ..
                },
                right: box Node::Intermediate {
                    free: Range { start: 13, end: 13 },
                    left: box Node::Leaf {
                        start: 10,
                        end: 13,
                        ..
                    },
                    right: box Node::Leaf {
                        start: 13,
                        end: 18,
                        ..
                    },
                },
            })
        );

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
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 18, end: 18 },
                left: box Node::Intermediate {
                    free: Range { start: 10, end: 10 },
                    left: box Node::Leaf {
                        start: 5,
                        end: 10,
                        ..
                    },
                    right: box Node::Intermediate {
                        free: Range { start: 13, end: 13 },
                        left: box Node::Leaf {
                            start: 10,
                            end: 13,
                            ..
                        },
                        right: box Node::Leaf {
                            start: 13,
                            end: 18,
                            ..
                        },
                    },
                },
                right: box Node::Leaf {
                    start: 18,
                    end: 20,
                    ..
                },
            })
        );

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
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 20, end: 21 },
                left: box Node::Intermediate {
                    free: Range { start: 18, end: 18 },
                    left: box Node::Intermediate {
                        free: Range { start: 10, end: 10 },
                        left: box Node::Leaf {
                            start: 5,
                            end: 10,
                            ..
                        },
                        right: box Node::Intermediate {
                            free: Range { start: 13, end: 13 },
                            left: box Node::Leaf {
                                start: 10,
                                end: 13,
                                ..
                            },
                            right: box Node::Leaf {
                                start: 13,
                                end: 18,
                                ..
                            },
                        },
                    },
                    right: box Node::Leaf {
                        start: 18,
                        end: 20,
                        ..
                    },
                },
                right: box Node::Intermediate {
                    free: Range { start: 23, end: 25 },
                    left: box Node::Leaf {
                        start: 21,
                        end: 23,
                        ..
                    },
                    right: box Node::Leaf {
                        start: 25,
                        end: 30,
                        ..
                    },
                },
            })
        );
    }

    #[test]
    fn test_unschedule() {
        let data = generate_data(10);

        let mut tree: ScheduleTree<i8, i8> = ScheduleTree::new();

        // 5..9
        // =>
        // <empty>
        tree.schedule_exact(5, 4, data[0]);
        let entry = tree.unschedule(&data[0]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 5,
                end: 9,
                ..
            })
        );
        assert_matches!(
            tree,
            ScheduleTree {
                root: None,
                scope: None,
                ..
            }
        );
        assert!(tree.data_map.is_empty());

        //   free:9..13
        //    /        \
        // 5..9       13..18
        // =>
        // 5..9
        tree.schedule_exact(5, 4, data[0]);
        tree.schedule_exact(13, 5, data[1]);
        let entry = tree.unschedule(&data[1]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 13,
                end: 18,
                ..
            })
        );
        assert_eq!(tree.scope, Some(5..9));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 5,
                end: 9,
                ..
            })
        );

        //   free:9..13
        //    /        \
        // 5..9       13..18
        // =>
        // 13..18
        tree.schedule_exact(5, 4, data[0]);
        tree.schedule_exact(13, 5, data[1]);
        let entry = tree.unschedule(&data[0]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 5,
                end: 9,
                ..
            })
        );
        assert_eq!(tree.scope, Some(13..18));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 13,
                end: 18,
                ..
            })
        );

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
        tree.schedule_close_before(9, 4, None, data[0]);
        tree.schedule_close_after(10, 2, None, data[2]);

        let entry = tree.unschedule(&data[0]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 5,
                end: 9,
                ..
            })
        );
        assert_eq!(tree.scope, Some(10..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 12, end: 13 },
                left: box Node::Leaf {
                    start: 10,
                    end: 12,
                    ..
                },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
            })
        );

        let entry = tree.unschedule(&data[2]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 10,
                end: 12,
                ..
            })
        );
        assert_eq!(tree.scope, Some(13..18));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 13,
                end: 18,
                ..
            })
        );

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
        tree.schedule_close_after(10, 2, None, data[0]);
        assert_eq!(tree.scope, Some(10..18));
        tree.schedule_close_before(9, 4, None, data[2]);
        assert_eq!(tree.scope, Some(5..18));

        let entry = tree.unschedule(&data[0]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 10,
                end: 12,
                ..
            })
        );
        assert_eq!(tree.scope, Some(5..18));
        assert_matches!(
            tree.root,
            Some(Node::Intermediate {
                free: Range { start: 9, end: 13 },
                left: box Node::Leaf {
                    start: 5,
                    end: 9,
                    ..
                },
                right: box Node::Leaf {
                    start: 13,
                    end: 18,
                    ..
                },
            })
        );

        let entry = tree.unschedule(&data[2]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 5,
                end: 9,
                ..
            })
        );
        assert_eq!(tree.scope, Some(13..18));
        assert_matches!(
            tree.root,
            Some(Node::Leaf {
                start: 13,
                end: 18,
                ..
            })
        );

        let entry = tree.unschedule(&data[1]);
        assert_matches!(
            entry,
            Some(Entry {
                start: 13,
                end: 18,
                ..
            })
        );
        assert_matches!(
            tree,
            ScheduleTree {
                root: None,
                scope: None,
                ..
            }
        );
        assert!(tree.data_map.is_empty());
    }

    fn generate_data(n: i8) -> Vec<i8> {
        (0..n).collect()
    }
}
