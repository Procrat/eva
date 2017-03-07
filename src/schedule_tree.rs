use std::fmt::Debug;
use std::ops::{Add, Range, Sub};

use take_mut;


#[derive(Debug)]
pub struct ScheduleTree<'a, T: 'a, D: 'a> {
    root: Option<Node<'a, T, D>>,
    scope: Option<Range<T>>,
}

#[derive(Debug, PartialEq)]
enum Node<'a, T: 'a, D: 'a> {
    Leaf { start: T, end: T, data: &'a D },
    Intermediate {
        left: Box<Node<'a, T, D>>,
        right: Box<Node<'a, T, D>>,
        free: Range<T>,
    },
}


impl<'a, T: Copy + Clone + Ord + Debug, D: Debug> ScheduleTree<'a, T, D> {
    pub fn new() -> Self {
        ScheduleTree {
            root: None,
            scope: None,
        }
    }

    pub fn schedule_exact<W>(&mut self, start: T, duration: W, data: &'a D) -> bool
        where T: Add<W, Output = T> + Sub<T, Output = W> + Sub<W, Output = T>,
              W: Add<W, Output = W> + Add<T, Output = T> + Sub<W, Output = W>
    {
        let end = start + duration;

        if let None = self.root {
            self.root = Some(Node::Leaf {
                start: start,
                end: end,
                data: data,
            });
            self.scope = Some(start..end);
            return true;
        }

        let scope = self.scope.as_ref().cloned().unwrap();
        if end <= scope.start {
            let new_node = Node::Leaf {
                start: start,
                end: end,
                data: data,
            };
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                left: Box::new(new_node),
                right: Box::new(root),
                free: end..scope.start,
            });
            self.scope = Some(start..scope.end);
            return true;
        } else if scope.end <= start {
            let new_node = Node::Leaf {
                start: start,
                end: end,
                data: data,
            };
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                left: Box::new(root),
                right: Box::new(new_node),
                free: scope.end..start,
            });
            self.scope = Some(scope.start..end);
            return true;
        }

        self.root.as_mut().unwrap().insert(start, end, data)
    }

    pub fn schedule_after<W>(&self, start: T, duration: W, max_end: Option<T>, data: &'a D) -> bool
        where T: Add<W, Output = T> + Sub<T, Output = W> + Sub<W, Output = T>,
              W: Add<W, Output = W> + Add<T, Output = T> + Sub<W, Output = W> + Copy
    {
        assert!(max_end.map_or(true, |max_end| start + duration <= max_end));
        // TODO schedule
        true
    }
}

impl<'a, T: Copy + Ord + Debug, D> Node<'a, T, D> {
    fn insert(&mut self, start: T, end: T, data: &'a D) -> bool {
        match *self {
            Node::Intermediate { ref mut left, ref mut right, ref mut free } => {
                if end <= free.start {
                    left.insert(start, end, data)
                } else if free.end <= start {
                    right.insert(start, end, data)
                } else if free.start <= start && end <= free.end {
                    // [start, end] completely within self.free
                    let new_node = Node::Leaf {
                        start: start,
                        end: end,
                        data: data,
                    };
                    take_mut::take(right, |right_value| {
                        Box::new(Node::Intermediate {
                            left: new_node.into(),
                            right: right_value,
                            free: end..free.end,
                        })
                    });
                    *free = free.start..start;
                    true
                } else {
                    // Overlap between [start, end] and self.free
                    false
                }
            }
            Node::Leaf { .. } => false,
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &i8 = &9;

    #[test]
    fn test_schedule_exact() {
        let mut tree: ScheduleTree<i8, i8> = ScheduleTree::new();
        let scheduled = tree.schedule_exact(5, 5, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..10));
        assert_matches!(tree.root, Some(Node::Leaf { start: 5, end: 10, ..}));

        let scheduled = tree.schedule_exact(12, 5, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..17));
        assert_matches!(tree.root, Some(Node::Intermediate { free: Range { start: 10, end: 12 }, .. }));
    }
}
