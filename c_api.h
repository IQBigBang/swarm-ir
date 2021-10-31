#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * Defines how the block is used
 */
typedef enum BlockTag {
  Undefined,
  /**
   * The "main" block of the function
   */
  Main,
  /**
   * A block which is used as one of the branches of an IfElse instruction
   */
  IfElse,
  /**
   * A block which is used as the body of a Loop instruction
   */
  Loop,
} BlockTag;

typedef enum Cmp {
  Eq,
  Ne,
  Lt,
  Le,
  Gt,
  Ge,
} Cmp;

typedef void *ModuleRef;

typedef const void *TypeRef;

typedef uintptr_t SMItemRef;

typedef void *FunctionBuilderRef;

/**
 * A wrapper which acts as a reference to a local.
 */
typedef uintptr_t LocalRef;

typedef uintptr_t BlockId;

ModuleRef create_module(void);

void free_module(ModuleRef module);

void dump_module(ModuleRef module);

TypeRef module_get_int32_type(ModuleRef module);

TypeRef module_get_uint32_type(ModuleRef module);

TypeRef module_get_int16_type(ModuleRef module);

TypeRef module_get_uint16_type(ModuleRef module);

TypeRef module_get_int8_type(ModuleRef module);

TypeRef module_get_uint8_type(ModuleRef module);

TypeRef module_get_float32_type(ModuleRef module);

TypeRef module_get_ptr_type(ModuleRef module);

TypeRef module_get_func_type(ModuleRef module,
                             const TypeRef *arg_types,
                             uintptr_t argc,
                             const TypeRef *ret_types,
                             uintptr_t retc);

TypeRef module_get_struct_type(ModuleRef module, const TypeRef *field_types, uintptr_t fieldc);

void module_new_int_global(ModuleRef module, const int8_t *global_name, int32_t value);

void module_new_float_global(ModuleRef module, const int8_t *global_name, float value);

void module_new_extern_function(ModuleRef module,
                                const int8_t *function_name,
                                TypeRef function_type);

/**
 * Add a blob of data into the static memory of the module
 */
SMItemRef module_new_static_memory_blob(ModuleRef module,
                                        const uint8_t *blob_ptr,
                                        uintptr_t blob_len,
                                        bool mutable_);

FunctionBuilderRef create_function_builder(const int8_t *function_name, TypeRef function_type);

void finish_function_builder(ModuleRef module, FunctionBuilderRef builder);

LocalRef builder_get_arg(FunctionBuilderRef builder, uintptr_t arg_index);

LocalRef builder_new_local(FunctionBuilderRef builder, TypeRef ty);

BlockId builder_new_block(FunctionBuilderRef builder,
                          const TypeRef *block_returns,
                          uintptr_t block_returnc,
                          enum BlockTag block_tag);

void builder_switch_block(FunctionBuilderRef builder, BlockId new_block);

BlockId builder_get_current_block(FunctionBuilderRef builder);

void builder_i_ld_int(FunctionBuilderRef builder, uint32_t val, TypeRef int_type);

void builder_i_ld_float(FunctionBuilderRef builder, float val);

void builder_i_iadd(FunctionBuilderRef builder);

void builder_i_isub(FunctionBuilderRef builder);

void builder_i_imul(FunctionBuilderRef builder);

void builder_i_idiv(FunctionBuilderRef builder);

void builder_i_fadd(FunctionBuilderRef builder);

void builder_i_fsub(FunctionBuilderRef builder);

void builder_i_fmul(FunctionBuilderRef builder);

void builder_i_fdiv(FunctionBuilderRef builder);

void builder_i_itof(FunctionBuilderRef builder);

void builder_i_not(FunctionBuilderRef builder);

void builder_i_bitand(FunctionBuilderRef builder);

void builder_i_bitor(FunctionBuilderRef builder);

void builder_i_call_indirect(FunctionBuilderRef builder);

void builder_i_memory_grow(FunctionBuilderRef builder);

void builder_i_memory_size(FunctionBuilderRef builder);

void builder_i_discard(FunctionBuilderRef builder);

void builder_i_return(FunctionBuilderRef builder);

void builder_i_fail(FunctionBuilderRef builder);

void builder_i_break(FunctionBuilderRef builder);

void builder_i_ftoi(FunctionBuilderRef builder, TypeRef int_type);

void builder_i_iconv(FunctionBuilderRef builder, TypeRef int_type);

void builder_i_icmp(FunctionBuilderRef builder, enum Cmp cmp);

void builder_i_fcmp(FunctionBuilderRef builder, enum Cmp cmp);

void builder_i_call(FunctionBuilderRef builder, const int8_t *func_name);

void builder_i_ld_local(FunctionBuilderRef builder, LocalRef loc);

void builder_i_st_local(FunctionBuilderRef builder, LocalRef loc);

void builder_i_ld_global_func(FunctionBuilderRef builder, const int8_t *func_name);

void builder_i_bitcast(FunctionBuilderRef builder, TypeRef target_type);

void builder_i_if(FunctionBuilderRef builder, BlockId then_block);

void builder_i_if_else(FunctionBuilderRef builder, BlockId then_block, BlockId else_block);

void builder_i_read(FunctionBuilderRef builder, TypeRef ty);

void builder_i_write(FunctionBuilderRef builder, TypeRef ty);

void builder_i_offset(FunctionBuilderRef builder, TypeRef ty);

void builder_i_get_field_ptr(FunctionBuilderRef builder, TypeRef struct_ty, uintptr_t field_idx);

void builder_i_ld_global(FunctionBuilderRef builder, const int8_t *name);

void builder_i_st_global(FunctionBuilderRef builder, const int8_t *name);

void builder_i_loop(FunctionBuilderRef builder, BlockId body_block);

void builder_i_ld_static_mem_ptr(FunctionBuilderRef builder, SMItemRef static_mem_item);

const uint8_t *compile_full_module(ModuleRef module, bool opt, uintptr_t *out_len);
