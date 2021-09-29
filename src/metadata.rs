use std::{any::Any, fmt::{Debug, Display}, marker::PhantomData};

use crate::{irprint::IRPrint, ty::Ty};

/// An efficient metadata key.
/// Obtained via the [`key!`] macro.
#[derive(Clone, Copy)]
pub(crate) struct Key(pub(crate) usize);

/// Get a metadata key
macro_rules! key {
    ("ty") => { crate::metadata::Key(0) };
    ("from") => { crate::metadata::Key(1) };
    ("parent") => { crate::metadata::Key(2) };
}

// The opposite of the macro
impl From<&Key> for &'static str {
    fn from(key: &Key) -> Self {
        match key.0 {
            0 => "ty",
            1 => "from",
            2 => "parent",
            _ => unreachable!()
        }
    }
}
impl std::fmt::Display for Key {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f, "{}", { let x: &'static str = self.into(); x }) }
}

/// Metadata can be indexed either by:
/// * &'static str - more comfortable, more prone to errors (typos) and less performant at runtime
/// * Key - less comfortable, is basically an integer
pub(crate) trait MetadataKey : Copy + Display {
    fn eq(self, other: Self) -> bool;
}
impl MetadataKey for &'static str {
    #[inline(always)]
    fn eq(self, other: Self) -> bool { self == other }
}
impl MetadataKey for Key {
    #[inline(always)]
    fn eq(self, other: Self) -> bool { self.0 == other.0 }
}

trait MetadataRequiredTraits {
    fn as_any(&self) -> &dyn Any;
    fn as_ir_print(&self) -> &dyn IRPrint;
    fn clone_into_box(&self) -> Box<dyn MetadataRequiredTraits>;
}

impl<T: Any + IRPrint + Clone> MetadataRequiredTraits for T {
    fn as_any(&self) -> &dyn Any { self }
    fn as_ir_print(&self) -> &dyn IRPrint { self }
    /// We can't have a &dyn Clone because it's not object safe, this is an alternative
    fn clone_into_box(&self) -> Box<dyn MetadataRequiredTraits> {
        Box::new(self.clone())
    }
}

/// These form a linked list
struct MetadataNode<K: MetadataKey> {
    key: K,
    val: Box<dyn MetadataRequiredTraits>,
    next: Option<Box<MetadataNode<K>>>
}

impl<K: MetadataKey> Clone for MetadataNode<K> {
    fn clone(&self) -> Self {
        MetadataNode {
            key: self.key,
            val: self.val.clone_into_box(),
            next: self.next.clone()
        }
    }
}

/// [`Metadata`] is a multi-threading-enabled dictionary of dynamically-typed values.
///
/// It is used to add notes/analysis results/type information to instructions, block, functions etc.
#[repr(transparent)]
#[allow(clippy::type_complexity)]
#[derive(Clone)]
pub(crate) struct Metadata<'ctx, K: MetadataKey>(Option<Box<MetadataNode<K>>>, PhantomData<Ty<'ctx>>);

impl<'ctx, K: MetadataKey> Metadata<'ctx, K> {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Metadata(None, PhantomData)
    }

    pub(crate) fn insert_ty(&mut self, name: K, value: Ty<'ctx>) {
        self.insert::<Ty<'static>>(name, unsafe { 
            std::mem::transmute::<Ty<'ctx>, Ty<'static>>(value) /* the type annotations are to make sure the transmute is correct */ 
        });
    }

    pub(crate) fn insert<T: Any + IRPrint + Clone>(&mut self, name: K, value: T) {
        let old_first = self.0.take();
        // create the MetadataNode
        let first = MetadataNode {
            key: name,
            val: Box::new(value) as Box<dyn MetadataRequiredTraits>,
            next: old_first
        };
        self.0 = Some(Box::new(first));
    }

    fn find_value<'a>(node: &'a MetadataNode<K>, key: K) -> Option<&'a dyn MetadataRequiredTraits> {
        let mut current = node;
        loop {
            if current.key.eq(key) {
                return Some(&*current.val)
            }
            if let Some(next) = &current.next {
                current = &*next;
            } else {
                return None
            }
        }
    }

    pub(crate) fn retrieve<T: Any>(&self, name: K) -> Option<&T> {
        match &self.0 {
            None => None, // no items => you can't retrieve anything
            Some(first) => {
                let retrieved = Metadata::find_value(&*first, name);
                match retrieved {
                    Some(obj) => {
                        obj.as_any().downcast_ref()
                    },
                    None => None
                }
            }
        }
    }

    pub(crate) fn retrieve_ty(&self, name: K) -> Option<Ty<'ctx>> {
        self.retrieve::<Ty<'static>>(name).map(|x| unsafe {
            std::mem::transmute::<Ty<'static>, Ty<'ctx>>(*x) /* the type annotations are to make sure the transmute is correct */ 
        })
    }

    pub(crate) fn retrieve_cloned<T: Any + Clone>(&self, name: K) -> Option<T> {
        self.retrieve(name).cloned()
    }

    pub(crate) fn retrieve_copied<T: Any + Copy>(&self, name: K) -> Option<T> {
        self.retrieve(name).copied()
    }

    /// Remove all keys and values and deallocate
    pub(crate) fn reset(&mut self) {
        std::mem::drop(self.0.take());
    }

    pub(crate) fn is_empty(&self) -> bool {
        matches!(self.0, None)
    }
}

impl<K: MetadataKey> Default for Metadata<'_, K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: MetadataKey> IRPrint for Metadata<'_, K> {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "{{")?;

        let mut current = &self.0;
        while let Some(node) = current {
            write!(w, "{}: ", node.key)?;
            node.val.as_ir_print().ir_print(w)?;

            if node.next.is_some() {
                write!(w, ", ")?;
            }
            current = &node.next;
        }

        write!(w, "}}")
    }
}

impl<K: MetadataKey> Debug for Metadata<'_, K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ir_print(f)
    }
}


#[cfg(test)]
mod tests {
    use crate::{irprint::IRPrint, module::{Module, WasmModuleConf}, ty::Type, ty::Ty};

    use super::Metadata;

    impl IRPrint for usize {
        fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
            write!(w, "{}", self)
        }
    }

    impl IRPrint for String {
        fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
            write!(w, "{}", self)
        }
    }

    #[test]
    fn metadata_test() {
        let mut meta = Metadata::new();
        meta.insert("x", 12usize);
        assert_eq!(meta.retrieve::<usize>("x"), Some(&12));

        meta.insert("greeting", String::from("Hello, world!"));
        assert_eq!(meta.retrieve_cloned::<String>("greeting").unwrap(), Some("Hello, world!").unwrap());

        assert_eq!(meta.retrieve::<usize>("y"), None);
    }

    #[test]
    fn metadata_ty_test() {
        let mut m = Module::new(WasmModuleConf::default());
        let mut meta = Metadata::new();

        meta.insert_ty("int", m.int32t());
        meta.insert_ty("flt", m.float32t());
        meta.insert_ty("fun", m.intern_type(Type::Func { args: vec![m.int32t()], ret: vec![m.int32t()] }));

        assert_eq!(meta.retrieve_ty("int"), Some(m.int32t()));
        assert_eq!(meta.retrieve_ty("flt"), Some(m.float32t()));
        assert_eq!(meta.retrieve_ty("fun"), Some(m.intern_type(Type::Func { args: vec![m.int32t()], ret: vec![m.int32t()] })));
    }

    #[test]
    fn metadata_ir_print_test() {
        let mut meta = Metadata::new();

        let mut out = String::new();

        meta.insert("x", 12usize);
        meta.insert("greeting", String::from("Hello, world!"));

        assert_eq!(meta.ir_print(&mut out), Ok(()));
        assert_eq!(
            out,
            "{greeting: Hello, world!, x: 12}"
        )
    }
}