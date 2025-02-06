use crate::Node;

use core::fmt::Debug;

pub trait NodeReferenceCollection<'a>: Default + Debug + PartialEq {
    type Error: Debug;
    type NodeReference: Debug + PartialEq;
    fn add(&mut self, reference: Self::NodeReference) -> Result<(), Self::Error>;
    fn len(&self) -> usize;
    fn iter(&self) -> Box<dyn Iterator<Item = &Self::NodeReference> + '_>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait NodeArena<'a> where Self: Sized {
    type Error: Debug;
    type NodeReference: Debug + PartialEq;
    type NodeReferenceList: NodeReferenceCollection<'a, NodeReference = Self::NodeReference>;
    fn add(&mut self, node: Node<'a, Self>) -> Result<Self::NodeReference, Self::Error>;
    fn len(&self) -> usize;
    fn get(&self, index: &Self::NodeReference) -> &super::Node<'a, Self>;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub mod vec {
    use std::{cell::{Cell, RefCell, UnsafeCell}, ops::Deref};

    use super::NodeArena;

    #[derive(PartialEq, Debug, Clone, Copy)]
    pub struct NodeIndex(usize);

    impl super::NodeReferenceCollection<'_> for Vec<NodeIndex> {
        type Error = ();
        type NodeReference = NodeIndex;
        fn add(&mut self, reference: Self::NodeReference) -> Result<(), Self::Error> {
            self.push(reference);
            Ok(())
        }
        fn len(&self) -> usize {
            self.len()
        }
        fn iter(&self) -> Box<dyn Iterator<Item = &Self::NodeReference> + '_> {
            Box::new(self[..].iter())
        }
        fn is_empty(&self) -> bool {
            self.is_empty()
        }
    }

    #[derive(PartialEq, Debug)]
    pub struct VecNodeArena<'a>(Vec<RefCell<Option<super::Node<'a, VecNodeArena<'a>>>>>);
    impl<'a> super::NodeArena<'a> for VecNodeArena<'a> {
        type Error = ();
        type NodeReference = NodeIndex;
        type NodeReferenceList = Vec<Self::NodeReference>;
        fn add(&mut self, node: crate::Node<'a, Self>) -> Result<Self::NodeReference, Self::Error> where Self: Sized {
            let index = NodeIndex(self.0.len());
            self.0.push(Some(node).into());
            Ok(index)
        }
        fn len(&self) -> usize {
            self.0.len()
        }
        fn get(&self, index: &Self::NodeReference) -> &super::Node<'a, Self> {
            unsafe { &* self.0.get(index.0).expect("invalid reference").as_ptr() }.as_ref().expect("taken")
        }
        fn is_empty(&self) -> bool {
            self.0.is_empty()
        }
    }
    impl<'a> VecNodeArena<'a> {
        pub fn new() -> Self {
            Self(vec![])
        }
    
        pub fn with_capacity(capacity: usize) -> Self {
            Self(Vec::with_capacity(capacity))
        }

        /// # Safety
        /// - Vector must be empty.
        pub unsafe fn using_vec(vec: Vec<RefCell<Option<super::Node<'a, VecNodeArena<'a>>>>>) -> Self {
            Self(vec)
        }

        pub fn take(&self, index: &<VecNodeArena<'a> as super::NodeArena::<'a>>::NodeReference) -> super::Node<'a, Self> {
            self.0.get(index.0).expect("invalid reference").take().expect("already taken")
        }
        pub fn replace(&self, index: &<VecNodeArena<'a> as super::NodeArena::<'a>>::NodeReference, node: super::Node<'a, Self>) -> Option<super::Node<'a, Self>> {
            self.0.get(index.0).expect("invalid reference").replace(Some(node))
        }
        pub fn count(&self) -> usize {
            self.0.len()
        }
    }
    impl Default for VecNodeArena<'_> {
        fn default() -> Self {
            Self::new()
        }
    }
}
