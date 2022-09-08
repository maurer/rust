use rustc_hir::ConstContext;
use rustc_macros::SessionDiagnostic;
use rustc_span::Span;

#[derive(SessionDiagnostic)]
#[diag(const_eval::unstable_in_stable)]
pub(crate) struct UnstableInStable {
    pub gate: String,
    #[primary_span]
    pub span: Span,
    #[suggestion(
        const_eval::unstable_sugg,
        code = "#[rustc_const_unstable(feature = \"...\", issue = \"...\")]\n",
        applicability = "has-placeholders"
    )]
    #[suggestion(
        const_eval::bypass_sugg,
        code = "#[rustc_allow_const_fn_unstable({gate})]\n",
        applicability = "has-placeholders"
    )]
    pub attr_span: Span,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::thread_local_access, code = "E0625")]
pub(crate) struct NonConstOpErr {
    #[primary_span]
    pub span: Span,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::static_access, code = "E0013")]
#[help]
pub(crate) struct StaticAccessErr {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
    #[note(const_eval::teach_note)]
    #[help(const_eval::teach_help)]
    pub teach: Option<()>,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::raw_ptr_to_int)]
#[note]
#[note(const_eval::note2)]
pub(crate) struct RawPtrToIntErr {
    #[primary_span]
    pub span: Span,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::raw_ptr_comparison)]
#[note]
pub(crate) struct RawPtrComparisonErr {
    #[primary_span]
    pub span: Span,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::panic_non_str)]
pub(crate) struct PanicNonStrErr {
    #[primary_span]
    pub span: Span,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::mut_deref, code = "E0658")]
pub(crate) struct MutDerefErr {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::transient_mut_borrow, code = "E0658")]
pub(crate) struct TransientMutBorrowErr {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::transient_mut_borrow_raw, code = "E0658")]
pub(crate) struct TransientMutBorrowErrRaw {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::max_num_nodes_in_const)]
pub(crate) struct MaxNumNodesInConstErr {
    #[primary_span]
    pub span: Span,
    pub global_const_id: String,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unallowed_fn_pointer_call)]
pub(crate) struct UnallowedFnPointerCall {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unstable_const_fn)]
pub(crate) struct UnstableConstFn {
    #[primary_span]
    pub span: Span,
    pub def_path: String,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unallowed_mutable_refs, code = "E0764")]
pub(crate) struct UnallowedMutableRefs {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
    #[note(const_eval::teach_note)]
    pub teach: Option<()>,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unallowed_mutable_refs_raw, code = "E0764")]
pub(crate) struct UnallowedMutableRefsRaw {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
    #[note(const_eval::teach_note)]
    pub teach: Option<()>,
}
#[derive(SessionDiagnostic)]
#[diag(const_eval::non_const_fmt_macro_call, code = "E0015")]
pub(crate) struct NonConstFmtMacroCall {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::non_const_fn_call, code = "E0015")]
pub(crate) struct NonConstFnCall {
    #[primary_span]
    pub span: Span,
    pub def_path_str: String,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unallowed_op_in_const_context)]
pub(crate) struct UnallowedOpInConstContext {
    #[primary_span]
    pub span: Span,
    pub msg: String,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unallowed_heap_allocations, code = "E0010")]
pub(crate) struct UnallowedHeapAllocations {
    #[primary_span]
    #[label]
    pub span: Span,
    pub kind: ConstContext,
    #[note(const_eval::teach_note)]
    pub teach: Option<()>,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::unallowed_inline_asm, code = "E0015")]
pub(crate) struct UnallowedInlineAsm {
    #[primary_span]
    pub span: Span,
    pub kind: ConstContext,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::interior_mutable_data_refer, code = "E0492")]
pub(crate) struct InteriorMutableDataRefer {
    #[primary_span]
    #[label]
    pub span: Span,
    #[help]
    pub opt_help: Option<()>,
    pub kind: ConstContext,
    #[note(const_eval::teach_note)]
    pub teach: Option<()>,
}

#[derive(SessionDiagnostic)]
#[diag(const_eval::interior_mutability_borrow)]
pub(crate) struct InteriorMutabilityBorrow {
    #[primary_span]
    pub span: Span,
}
