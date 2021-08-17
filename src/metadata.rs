use std::{any::Any, fmt::Debug};

use crate::{irprint::IRPrint, ty::Ty};

//pub type Metadata = ();

/*trait Metadata {
    
}*/

trait MetadataRequiredTraits {
    fn as_any(&self) -> &dyn Any;
    fn as_ir_print(&self) -> &dyn IRPrint;
}
impl<T: Any + IRPrint> MetadataRequiredTraits for T {
    fn as_any(&self) -> &dyn Any { self }
    fn as_ir_print(&self) -> &dyn IRPrint { self }
}

/// These form a linked list
struct MetadataNode {
    key: &'static str,
    val: Box<dyn MetadataRequiredTraits>,
    next: Option<Box<MetadataNode>>
}

/// [`Metadata`] is a multi-threading-enabled dictionary of dynamically-typed values.
///
/// It is used to add notes/analysis results/type information to instructions, block, functions etc.
#[repr(transparent)]
#[allow(clippy::type_complexity)]
pub(crate) struct Metadata(Option<Box<MetadataNode>>);

impl Metadata {
    #[inline(always)]
    pub(crate) fn new() -> Self {
        Metadata(None)
    }

    pub(crate) fn insert_ty(&mut self, name: &'static str, value: Ty<'_>) {
        self.insert::<Ty<'static>>(name, unsafe { 
            std::mem::transmute::<Ty<'_>, Ty<'static>>(value) /* the type annotations are to make sure the transmute is correct */ 
        });
    }

    pub(crate) fn insert<T: Any + IRPrint>(&mut self, name: &'static str, value: T) {
        let old_first = self.0.take();
        // create the MetadataNode
        let first = MetadataNode {
            key: name,
            val: Box::new(value) as Box<dyn MetadataRequiredTraits>,
            next: old_first
        };
        self.0 = Some(Box::new(first));
    }

    fn find_value<'a>(node: &'a MetadataNode, key: &'static str) -> Option<&'a Box<dyn MetadataRequiredTraits>> {
        let mut current = node;
        loop {
            if current.key == key {
                return Some(&current.val)
            }
            if let Some(next) = &current.next {
                current = &*next;
            } else {
                return None
            }
        }
    }

    pub(crate) fn retrieve<T: Any>(&self, name: &'static str) -> Option<&T> {
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

    pub(crate) fn retrieve_ty<'ctx>(&self, name: &'static str) -> Option<Ty<'ctx>> {
        self.retrieve::<Ty<'static>>(name).map(|x| unsafe {
            std::mem::transmute::<Ty<'static>, Ty<'ctx>>(*x) /* the type annotations are to make sure the transmute is correct */ 
        })
    }

    pub(crate) fn retrieve_cloned<T: Any + Clone>(&self, name: &'static str) -> Option<T> {
        match &self.0 {
            None => None, // no items => you can't retrieve anything
            Some(first) => {
                let retrieved = Metadata::find_value(&*first, name);
                match retrieved {
                    Some(obj) => {
                        obj.as_any().downcast_ref().cloned()
                    },
                    None => None
                }
            }
        }
    }

    /// Remove all keys and values and deallocate
    pub(crate) fn reset(&mut self) {
        std::mem::drop(self.0.take());
    }

    pub(crate) fn is_empty(&self) -> bool {
        matches!(self.0, None)
    }
}

impl Default for Metadata {
    fn default() -> Self {
        Self::new()
    }
}

impl IRPrint for Metadata {
    fn ir_print(&self, w: &mut dyn std::fmt::Write) -> std::fmt::Result {
        write!(w, "{{")?;

        let mut current = &self.0;
        while let Some(node) = current {
            write!(w, "{}: ", node.key)?;
            node.val.as_ir_print().ir_print(w)?;
            write!(w, ", ")?;
            current = &node.next;
        }

        write!(w, "}}")
    }
}

impl Debug for Metadata {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.ir_print(f)
    }
}


#[cfg(test)]
mod tests {
    use crate::{irprint::IRPrint, module::{Module, WasmModuleConf}, ty::Type};

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
            "{greeting: Hello, world!, x: 12, }"
        )
    }
}