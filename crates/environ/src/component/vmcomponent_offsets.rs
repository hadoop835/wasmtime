// Currently the `VMComponentContext` allocation by field looks like this:
//
// struct VMComponentContext {
//      magic: u32,
//      libcalls: &'static VMComponentLibcalls,
//      store: *mut dyn Store,
//      limits: *const VMRuntimeLimits,
//      flags: [VMGlobalDefinition; component.num_runtime_component_instances],
//      lowering_func_refs: [VMFuncRef; component.num_lowerings],
//      always_trap_func_refs: [VMFuncRef; component.num_always_trap],
//      transcoder_func_refs: [VMFuncRef; component.num_transcoders],
//      resource_new_func_refs: [VMFuncRef; component.num_resource_new],
//      resource_rep_func_refs: [VMFuncRef; component.num_resource_rep],
//      resource_drop_func_refs: [VMFuncRef; component.num_resource_drop],
//      lowerings: [VMLowering; component.num_lowerings],
//      memories: [*mut VMMemoryDefinition; component.num_memories],
//      reallocs: [*mut VMFuncRef; component.num_reallocs],
//      post_returns: [*mut VMFuncRef; component.num_post_returns],
//      resource_destructors: [*mut VMFuncRef; component.num_resources],
// }

use crate::component::*;
use crate::PtrSize;

/// Equivalent of `VMCONTEXT_MAGIC` except for components.
///
/// This is stored at the start of all `VMComponentContext` structures and
/// double-checked on `VMComponentContext::from_opaque`.
pub const VMCOMPONENT_MAGIC: u32 = u32::from_le_bytes(*b"comp");

/// Flag for the `VMComponentContext::flags` field which corresponds to the
/// canonical ABI flag `may_leave`
pub const FLAG_MAY_LEAVE: i32 = 1 << 0;

/// Flag for the `VMComponentContext::flags` field which corresponds to the
/// canonical ABI flag `may_enter`
pub const FLAG_MAY_ENTER: i32 = 1 << 1;

/// Flag for the `VMComponentContext::flags` field which is set whenever a
/// function is called to indicate that `post_return` must be called next.
pub const FLAG_NEEDS_POST_RETURN: i32 = 1 << 2;

/// Runtime offsets within a `VMComponentContext` for a specific component.
#[derive(Debug, Clone, Copy)]
pub struct VMComponentOffsets<P> {
    /// The host pointer size
    pub ptr: P,

    /// The number of lowered functions this component will be creating.
    pub num_lowerings: u32,
    /// The number of memories which are recorded in this component for options.
    pub num_runtime_memories: u32,
    /// The number of reallocs which are recorded in this component for options.
    pub num_runtime_reallocs: u32,
    /// The number of post-returns which are recorded in this component for options.
    pub num_runtime_post_returns: u32,
    /// Number of component instances internally in the component (always at
    /// least 1).
    pub num_runtime_component_instances: u32,
    /// Number of "always trap" functions which have their
    /// `VMFuncRef` stored inline in the `VMComponentContext`.
    pub num_always_trap: u32,
    /// Number of transcoders needed for string conversion.
    pub num_transcoders: u32,
    /// Number of `resource.new` intrinsics within a component.
    pub num_resource_new: u32,
    /// Number of `resource.rep` intrinsics within a component.
    pub num_resource_rep: u32,
    /// Number of `resource.drop` intrinsics within a component.
    pub num_resource_drop: u32,
    /// Number of resources within a component which need destructors stored.
    pub num_resources: u32,

    // precalculated offsets of various member fields
    magic: u32,
    libcalls: u32,
    store: u32,
    limits: u32,
    flags: u32,
    lowering_func_refs: u32,
    always_trap_func_refs: u32,
    transcoder_func_refs: u32,
    resource_new_func_refs: u32,
    resource_rep_func_refs: u32,
    resource_drop_func_refs: u32,
    lowerings: u32,
    memories: u32,
    reallocs: u32,
    post_returns: u32,
    resource_destructors: u32,
    size: u32,
}

#[inline]
fn align(offset: u32, align: u32) -> u32 {
    assert!(align.is_power_of_two());
    (offset + (align - 1)) & !(align - 1)
}

impl<P: PtrSize> VMComponentOffsets<P> {
    /// Creates a new set of offsets for the `component` specified configured
    /// additionally for the `ptr` size specified.
    pub fn new(ptr: P, component: &Component) -> Self {
        let mut ret = Self {
            ptr,
            num_lowerings: component.num_lowerings.try_into().unwrap(),
            num_runtime_memories: component.num_runtime_memories.try_into().unwrap(),
            num_runtime_reallocs: component.num_runtime_reallocs.try_into().unwrap(),
            num_runtime_post_returns: component.num_runtime_post_returns.try_into().unwrap(),
            num_runtime_component_instances: component
                .num_runtime_component_instances
                .try_into()
                .unwrap(),
            num_always_trap: component.num_always_trap,
            num_transcoders: component.num_transcoders,
            num_resource_new: component.num_resource_new,
            num_resource_rep: component.num_resource_rep,
            num_resource_drop: component.num_resource_drop,
            num_resources: component.num_resources,
            magic: 0,
            libcalls: 0,
            store: 0,
            limits: 0,
            flags: 0,
            lowering_func_refs: 0,
            always_trap_func_refs: 0,
            transcoder_func_refs: 0,
            resource_new_func_refs: 0,
            resource_rep_func_refs: 0,
            resource_drop_func_refs: 0,
            lowerings: 0,
            memories: 0,
            reallocs: 0,
            post_returns: 0,
            resource_destructors: 0,
            size: 0,
        };

        // Convenience functions for checked addition and multiplication.
        // As side effect this reduces binary size by using only a single
        // `#[track_caller]` location for each function instead of one for
        // each individual invocation.
        #[inline]
        fn cmul(count: u32, size: u8) -> u32 {
            count.checked_mul(u32::from(size)).unwrap()
        }

        let mut next_field_offset = 0;

        macro_rules! fields {
            (size($field:ident) = $size:expr, $($rest:tt)*) => {
                ret.$field = next_field_offset;
                next_field_offset = next_field_offset.checked_add(u32::from($size)).unwrap();
                fields!($($rest)*);
            };
            (align($align:expr), $($rest:tt)*) => {
                next_field_offset = align(next_field_offset, $align);
                fields!($($rest)*);
            };
            () => {};
        }

        fields! {
            size(magic) = 4u32,
            align(u32::from(ret.ptr.size())),
            size(libcalls) = ret.ptr.size(),
            size(store) = cmul(2, ret.ptr.size()),
            size(limits) = ret.ptr.size(),
            align(16),
            size(flags) = cmul(ret.num_runtime_component_instances, ret.ptr.size_of_vmglobal_definition()),
            align(u32::from(ret.ptr.size())),
            size(lowering_func_refs) = cmul(ret.num_lowerings, ret.ptr.size_of_vm_func_ref()),
            size(always_trap_func_refs) = cmul(ret.num_always_trap, ret.ptr.size_of_vm_func_ref()),
            size(transcoder_func_refs) = cmul(ret.num_transcoders, ret.ptr.size_of_vm_func_ref()),
            size(resource_new_func_refs) = cmul(ret.num_resource_new, ret.ptr.size_of_vm_func_ref()),
            size(resource_rep_func_refs) = cmul(ret.num_resource_rep, ret.ptr.size_of_vm_func_ref()),
            size(resource_drop_func_refs) = cmul(ret.num_resource_drop, ret.ptr.size_of_vm_func_ref()),
            size(lowerings) = cmul(ret.num_lowerings, ret.ptr.size() * 2),
            size(memories) = cmul(ret.num_runtime_memories, ret.ptr.size()),
            size(reallocs) = cmul(ret.num_runtime_reallocs, ret.ptr.size()),
            size(post_returns) = cmul(ret.num_runtime_post_returns, ret.ptr.size()),
            size(resource_destructors) = cmul(ret.num_resources, ret.ptr.size()),
        }

        ret.size = next_field_offset;

        // This is required by the implementation of
        // `VMComponentContext::from_opaque`. If this value changes then this
        // location needs to be updated.
        assert_eq!(ret.magic, 0);

        return ret;
    }

    /// The size, in bytes, of the host pointer.
    #[inline]
    pub fn pointer_size(&self) -> u8 {
        self.ptr.size()
    }

    /// The offset of the `magic` field.
    #[inline]
    pub fn magic(&self) -> u32 {
        self.magic
    }

    /// The offset of the `libcalls` field.
    #[inline]
    pub fn libcalls(&self) -> u32 {
        self.libcalls
    }

    /// The offset of the `flags` field.
    #[inline]
    pub fn instance_flags(&self, index: RuntimeComponentInstanceIndex) -> u32 {
        assert!(index.as_u32() < self.num_runtime_component_instances);
        self.flags + index.as_u32() * u32::from(self.ptr.size_of_vmglobal_definition())
    }

    /// The offset of the `store` field.
    #[inline]
    pub fn store(&self) -> u32 {
        self.store
    }

    /// The offset of the `limits` field.
    #[inline]
    pub fn limits(&self) -> u32 {
        self.limits
    }

    /// The offset of the `lowering_func_refs` field.
    #[inline]
    pub fn lowering_func_refs(&self) -> u32 {
        self.lowering_func_refs
    }

    /// The offset of `VMFuncRef` for the `index` specified.
    #[inline]
    pub fn lowering_func_ref(&self, index: LoweredIndex) -> u32 {
        assert!(index.as_u32() < self.num_lowerings);
        self.lowering_func_refs() + index.as_u32() * u32::from(self.ptr.size_of_vm_func_ref())
    }

    /// The offset of the `always_trap_func_refs` field.
    #[inline]
    pub fn always_trap_func_refs(&self) -> u32 {
        self.always_trap_func_refs
    }

    /// The offset of `VMFuncRef` for the `index` specified.
    #[inline]
    pub fn always_trap_func_ref(&self, index: RuntimeAlwaysTrapIndex) -> u32 {
        assert!(index.as_u32() < self.num_always_trap);
        self.always_trap_func_refs() + index.as_u32() * u32::from(self.ptr.size_of_vm_func_ref())
    }

    /// The offset of the `transcoder_func_refs` field.
    #[inline]
    pub fn transcoder_func_refs(&self) -> u32 {
        self.transcoder_func_refs
    }

    /// The offset of `VMFuncRef` for the `index` specified.
    #[inline]
    pub fn transcoder_func_ref(&self, index: RuntimeTranscoderIndex) -> u32 {
        assert!(index.as_u32() < self.num_transcoders);
        self.transcoder_func_refs() + index.as_u32() * u32::from(self.ptr.size_of_vm_func_ref())
    }

    /// The offset of the `resource_new_func_refs` field.
    #[inline]
    pub fn resource_new_func_refs(&self) -> u32 {
        self.resource_new_func_refs
    }

    /// The offset of `VMFuncRef` for the `index` specified.
    #[inline]
    pub fn resource_new_func_ref(&self, index: RuntimeResourceNewIndex) -> u32 {
        assert!(index.as_u32() < self.num_resource_new);
        self.resource_new_func_refs() + index.as_u32() * u32::from(self.ptr.size_of_vm_func_ref())
    }

    /// The offset of the `resource_rep_func_refs` field.
    #[inline]
    pub fn resource_rep_func_refs(&self) -> u32 {
        self.resource_rep_func_refs
    }

    /// The offset of `VMFuncRef` for the `index` specified.
    #[inline]
    pub fn resource_rep_func_ref(&self, index: RuntimeResourceRepIndex) -> u32 {
        assert!(index.as_u32() < self.num_resource_rep);
        self.resource_rep_func_refs() + index.as_u32() * u32::from(self.ptr.size_of_vm_func_ref())
    }

    /// The offset of the `resource_drop_func_refs` field.
    #[inline]
    pub fn resource_drop_func_refs(&self) -> u32 {
        self.resource_drop_func_refs
    }

    /// The offset of `VMFuncRef` for the `index` specified.
    #[inline]
    pub fn resource_drop_func_ref(&self, index: RuntimeResourceDropIndex) -> u32 {
        assert!(index.as_u32() < self.num_resource_drop);
        self.resource_drop_func_refs() + index.as_u32() * u32::from(self.ptr.size_of_vm_func_ref())
    }

    /// The offset of the `lowerings` field.
    #[inline]
    pub fn lowerings(&self) -> u32 {
        self.lowerings
    }

    /// The offset of the `VMLowering` for the `index` specified.
    #[inline]
    pub fn lowering(&self, index: LoweredIndex) -> u32 {
        assert!(index.as_u32() < self.num_lowerings);
        self.lowerings() + index.as_u32() * u32::from(2 * self.ptr.size())
    }

    /// The offset of the `callee` for the `index` specified.
    #[inline]
    pub fn lowering_callee(&self, index: LoweredIndex) -> u32 {
        self.lowering(index) + self.lowering_callee_offset()
    }

    /// The offset of the `data` for the `index` specified.
    #[inline]
    pub fn lowering_data(&self, index: LoweredIndex) -> u32 {
        self.lowering(index) + self.lowering_data_offset()
    }

    /// The size of the `VMLowering` type
    #[inline]
    pub fn lowering_size(&self) -> u8 {
        2 * self.ptr.size()
    }

    /// The offset of the `callee` field within the `VMLowering` type.
    #[inline]
    pub fn lowering_callee_offset(&self) -> u32 {
        0
    }

    /// The offset of the `data` field within the `VMLowering` type.
    #[inline]
    pub fn lowering_data_offset(&self) -> u32 {
        u32::from(self.ptr.size())
    }

    /// The offset of the base of the `runtime_memories` field
    #[inline]
    pub fn runtime_memories(&self) -> u32 {
        self.memories
    }

    /// The offset of the `*mut VMMemoryDefinition` for the runtime index
    /// provided.
    #[inline]
    pub fn runtime_memory(&self, index: RuntimeMemoryIndex) -> u32 {
        assert!(index.as_u32() < self.num_runtime_memories);
        self.runtime_memories() + index.as_u32() * u32::from(self.ptr.size())
    }

    /// The offset of the base of the `runtime_reallocs` field
    #[inline]
    pub fn runtime_reallocs(&self) -> u32 {
        self.reallocs
    }

    /// The offset of the `*mut VMFuncRef` for the runtime index
    /// provided.
    #[inline]
    pub fn runtime_realloc(&self, index: RuntimeReallocIndex) -> u32 {
        assert!(index.as_u32() < self.num_runtime_reallocs);
        self.runtime_reallocs() + index.as_u32() * u32::from(self.ptr.size())
    }

    /// The offset of the base of the `runtime_post_returns` field
    #[inline]
    pub fn runtime_post_returns(&self) -> u32 {
        self.post_returns
    }

    /// The offset of the `*mut VMFuncRef` for the runtime index
    /// provided.
    #[inline]
    pub fn runtime_post_return(&self, index: RuntimePostReturnIndex) -> u32 {
        assert!(index.as_u32() < self.num_runtime_post_returns);
        self.runtime_post_returns() + index.as_u32() * u32::from(self.ptr.size())
    }

    /// The offset of the base of the `resource_destructors` field
    #[inline]
    pub fn resource_destructors(&self) -> u32 {
        self.resource_destructors
    }

    /// The offset of the `*mut VMFuncRef` for the runtime index
    /// provided.
    #[inline]
    pub fn resource_destructor(&self, index: ResourceIndex) -> u32 {
        assert!(index.as_u32() < self.num_resources);
        self.resource_destructors() + index.as_u32() * u32::from(self.ptr.size())
    }

    /// Return the size of the `VMComponentContext` allocation.
    #[inline]
    pub fn size_of_vmctx(&self) -> u32 {
        self.size
    }
}
