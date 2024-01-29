//! This pass rewrites the Self type of the receiver on a function to the provided type.

use rustc_middle::mir::*;
use rustc_middle::ty::{self, Ty, TyCtxt};

// FIXME this is a layering violation - this is replicating work that occurs when computing an ABI
// It's not immediately obvious to me why this doesn't break for a VTableShim around an Arc<Self>,
// does it just not happen?
pub fn force_thin_self_ptr<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Ty<'tcx> {
    use ty::layout::{LayoutCx, LayoutOf, MaybeResult, TyAndLayout};
    let cx = LayoutCx { tcx, param_env: ty::ParamEnv::reveal_all() };
    let mut receiver_layout: TyAndLayout<'_> =
        cx.layout_of(ty).to_result().expect("unable to compute layout of receiver type");
    // The VTableShim should have already done any `dyn Foo` -> `*const dyn Foo` coercions
    assert!(!receiver_layout.is_unsized());
    // If we aren't a pointer or a ref already, we better be a no-padding wrapper around one
    while !receiver_layout.ty.is_unsafe_ptr() && !receiver_layout.ty.is_ref() {
        receiver_layout = receiver_layout
            .non_1zst_field(&cx)
            .expect("not exactly one non-1-ZST field in a CFI shim receiver")
            .1
    }
    receiver_layout.ty
}

// Visitor to rewrite all uses of a given local to another
struct RewriteLocal<'tcx> {
    tcx: TyCtxt<'tcx>,
    source: Local,
    target: Local,
}

impl<'tcx> visit::MutVisitor<'tcx> for RewriteLocal<'tcx> {
    fn tcx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
    fn visit_local(
        &mut self,
        local: &mut Local,
        _context: visit::PlaceContext,
        _location: Location,
    ) {
        if self.source == *local {
            *local = self.target;
        }
    }
}

pub struct RewriteReceiver<'tcx> {
    invoke_ty: Ty<'tcx>,
}

impl<'tcx> RewriteReceiver<'tcx> {
    pub fn new(invoke_ty: Ty<'tcx>) -> Self {
        Self { invoke_ty }
    }
}

impl<'tcx> MirPass<'tcx> for RewriteReceiver<'tcx> {
    fn is_enabled(&self, sess: &rustc_session::Session) -> bool {
        sess.cfi_shims()
    }
    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        use visit::MutVisitor;
        let source_info = SourceInfo::outermost(body.span);
        let receiver =
            body.args_iter().next().expect("RewriteReceiver pass on function with no arguments?");
        let cast_receiver = body.local_decls.push(body.local_decls[receiver].clone());
        body.local_decls[receiver].ty = self.invoke_ty;
        body.local_decls[receiver].mutability = Mutability::Not;
        RewriteLocal { tcx, source: receiver, target: cast_receiver }.visit_body(body);
        body.basic_blocks.as_mut_preserves_cfg()[START_BLOCK].statements.insert(
            0,
            Statement {
                source_info,
                kind: StatementKind::Assign(Box::new((
                    Place::from(cast_receiver),
                    Rvalue::Cast(
                        CastKind::Transmute,
                        Operand::Move(Place::from(receiver)),
                        body.local_decls[cast_receiver].ty,
                    ),
                ))),
            },
        );
    }
}
