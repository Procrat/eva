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
        left: Box<Node<'a, T, D>>,
        right: Box<Node<'a, T, D>>,
        free: Range<T>,
    },
}

impl<'a, T, D> ScheduleTree<'a, T, D>
    where T: Copy + Clone + Ord + Debug,
          D: Debug
{
    pub fn new() -> Self {
        ScheduleTree {
            root: None,
            scope: None,
        }
    }

    pub fn iter<'b>(&'b self) -> Iter<'b, 'a, T, D> {
        Iter { path: self.root.iter().collect() }
    }

    pub fn schedule_exact<W>(&mut self, start: T, duration: W, data: &'a D) -> bool
        where T: Add<W, Output = T>
    {
        let end = start + duration;
        let new_node = Node::Leaf {
            start: start,
            end: end,
            data: data,
        };

        if self.root.is_none() {
            self.root = Some(new_node);
            self.scope = Some(start..end);
            return true;
        }

        let scope = self.scope
            .as_ref()
            .cloned()
            .unwrap();
        if end <= scope.start {
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                                 left: Box::new(new_node),
                                 right: Box::new(root),
                                 free: end..scope.start,
                             });
            self.scope = Some(start..scope.end);
            return true;
        } else if scope.end <= start {
            let root = self.root.take().unwrap();
            self.root = Some(Node::Intermediate {
                                 left: Box::new(root),
                                 right: Box::new(new_node),
                                 free: scope.end..start,
                             });
            self.scope = Some(scope.start..end);
            return true;
        }

        self.root
            .as_mut()
            .unwrap()
            .insert(start, end, new_node)
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

impl<'a, T, D> Node<'a, T, D>
    where T: Copy + Ord + Debug
{
    fn insert(&mut self, start: T, end: T, sub_tree: Node<'a, T, D>) -> bool {
        match *self {
            Node::Intermediate { ref mut left, ref mut right, ref mut free } => {
                if end <= free.start {
                    left.insert(start, end, sub_tree)
                } else if free.end <= start {
                    right.insert(start, end, sub_tree)
                } else if free.start <= start && end <= free.end {
                    // [start, end] completely within self.free
                    take_mut::take(right, |right_value| {
                        Box::new(Node::Intermediate {
                                     left: Box::new(sub_tree),
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
        self.path.pop().and_then(|mut current| {
            while let Node::Intermediate { ref left, ref right, .. } = *current {
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
        let mut tree: ScheduleTree<i8, i8> = ScheduleTree::new();

        // 5..10
        let scheduled = tree.schedule_exact(5, 5, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..10));
        assert_matches!(tree.root, Some(Node::Leaf { start: 5, end: 10, .. }));

        //   free:10..13
        //    /        \
        // 5..10      13..18
        let scheduled = tree.schedule_exact(13, 5, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 13 },
            right: box Node::Leaf { start: 13, end: 18, .. },
        .. }));

        //   free:10..10
        //    /        \
        // 5..10     free:12..13
        //             /     \
        //          10..12  13..18
        let scheduled = tree.schedule_exact(10, 2, DATA);
        assert!(scheduled);
        assert!(tree.scope == Some(5..18));
        assert_matches!(tree.root, Some(Node::Intermediate {
            free: Range { start: 10, end: 10 },
            right: box Node::Intermediate {
                free: Range { start: 12, end: 13 },
                left: box Node::Leaf { start: 10, end: 12, .. },
            .. },
        .. }));
    }
}
