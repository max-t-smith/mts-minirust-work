use crate::build::*;

impl FunctionBuilder {
    #[track_caller]
    fn finish_block(&mut self, terminator: Terminator) {
        let cur_block = self.cur_block.take().expect("finish_block: there is no block to finish");
        let bb = BasicBlock { statements: cur_block.statements, terminator, kind: cur_block.kind };
        self.blocks.try_insert(cur_block.name, bb).unwrap();
    }

    /// Finishes the current block and creates a new block of the same block kind.
    fn finish_with_next_block<F>(&mut self, builder: F)
    where
        F: FnOnce(BbName) -> Terminator,
    {
        let block_kind = self.cur_block().kind;
        let next_block = self.declare_block();
        let terminator = builder(next_block);
        self.finish_block(terminator);
        self.set_cur_block(next_block, block_kind);
    }

    // terminators with 0 following blocks

    pub fn exit(&mut self) {
        self.finish_block(exit());
    }

    pub fn abort(&mut self) {
        self.finish_block(abort());
    }

    pub fn unreachable(&mut self) {
        self.finish_block(Terminator::Unreachable);
    }

    pub fn return_(&mut self) {
        self.finish_block(Terminator::Return);
    }

    pub fn resume_unwind(&mut self) {
        self.finish_block(Terminator::ResumeUnwind);
    }

    // Call terminators

    /// This is a helper function that handles function calls.
    fn handle_call(
        &mut self,
        ret: PlaceExpr,
        f: ValueExpr,
        args: &[ArgumentExpr],
        conv: CallingConvention,
        next_block: Option<BbName>,
        unwind_block: Option<BbName>,
    ) {
        let block_kind = self.cur_block().kind;
        self.finish_block(Terminator::Call {
            callee: f,
            calling_convention: conv,
            arguments: args.iter().copied().collect(),
            ret,
            next_block,
            unwind_block,
        });
        if let Some(next_block) = next_block {
            self.set_cur_block(next_block, block_kind);
        }
    }

    /// Calls a function that neither returns nor unwinds using the Rust calling convention.
    pub fn call_noret(&mut self, ret: PlaceExpr, f: ValueExpr, args: &[ArgumentExpr]) {
        self.handle_call(ret, f, args, CallingConvention::Rust, None, None);
    }

    /// Call a function that does not unwind using the Rust calling convention.
    pub fn call_nounwind(&mut self, ret: PlaceExpr, f: ValueExpr, args: &[ArgumentExpr]) {
        let next_block = self.declare_block();
        self.handle_call(ret, f, args, CallingConvention::Rust, Some(next_block), None);
    }

    /// Call a function that does not unwind using the Rust calling convention. Ignore unit type return value.
    pub fn call_ignoreret(&mut self, f: ValueExpr, args: &[ArgumentExpr]) {
        let next_block = self.declare_block();
        self.handle_call(unit_place(), f, args, CallingConvention::Rust, Some(next_block), None);
    }

    /// Call a function using the Rust calling convention.
    pub fn call(
        &mut self,
        ret: PlaceExpr,
        f: ValueExpr,
        args: &[ArgumentExpr],
        unwind_block: BbName,
    ) {
        let next_block = self.declare_block();
        self.handle_call(
            ret,
            f,
            args,
            CallingConvention::Rust,
            Some(next_block),
            Some(unwind_block),
        );
    }

    /// Call a function using the calling convention determined by `conv`.
    pub fn call_with_conv(
        &mut self,
        ret: PlaceExpr,
        f: ValueExpr,
        args: &[ArgumentExpr],
        conv: CallingConvention,
        unwind_block: BbName,
    ) {
        let next_block = self.declare_block();
        self.handle_call(ret, f, args, conv, Some(next_block), Some(unwind_block));
    }

    // terminators with 1 following block

    pub fn goto(&mut self, dest: BbName) {
        self.finish_block(Terminator::Goto(dest));
    }

    pub fn assume(&mut self, val: ValueExpr) {
        self.finish_with_next_block(|next_block| assume(val, bbname_into_u32(next_block)));
    }

    pub fn print(&mut self, arg: ValueExpr) {
        self.finish_with_next_block(|next_block| print(arg, bbname_into_u32(next_block)));
    }

    pub fn eprint(&mut self, arg: ValueExpr) {
        self.finish_with_next_block(|next_block| eprint(arg, bbname_into_u32(next_block)));
    }

    pub fn allocate(&mut self, size: ValueExpr, align: ValueExpr, ret_place: PlaceExpr) {
        self.finish_with_next_block(|next_block| {
            allocate(size, align, ret_place, bbname_into_u32(next_block))
        });
    }

    pub fn deallocate(&mut self, ptr: ValueExpr, size: ValueExpr, align: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            deallocate(ptr, size, align, bbname_into_u32(next_block))
        });
    }

    pub fn spawn(&mut self, f: FnName, data_ptr: ValueExpr, ret: PlaceExpr) {
        self.finish_with_next_block(|next_block| {
            spawn(fn_ptr(f), data_ptr, ret, bbname_into_u32(next_block))
        });
    }

    pub fn join(&mut self, thread_id: ValueExpr) {
        self.finish_with_next_block(|next_block| join(thread_id, bbname_into_u32(next_block)));
    }

    pub fn raw_eq(&mut self, dest: PlaceExpr, left_ptr: ValueExpr, right_ptr: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            raw_eq(dest, left_ptr, right_ptr, bbname_into_u32(next_block))
        });
    }

    pub fn atomic_store(&mut self, ptr: ValueExpr, src: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            atomic_store(ptr, src, bbname_into_u32(next_block))
        });
    }

    pub fn atomic_load(&mut self, dest: PlaceExpr, ptr: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            atomic_load(dest, ptr, bbname_into_u32(next_block))
        });
    }

    pub fn atomic_fetch(
        &mut self,
        binop: FetchBinOp,
        dest: PlaceExpr,
        ptr: ValueExpr,
        other: ValueExpr,
    ) {
        self.finish_with_next_block(|next_block| {
            atomic_fetch(binop, dest, ptr, other, bbname_into_u32(next_block))
        });
    }

    pub fn compare_exchange(
        &mut self,
        dest: PlaceExpr,
        ptr: ValueExpr,
        current: ValueExpr,
        next_val: ValueExpr,
    ) {
        self.finish_with_next_block(|next_block| {
            compare_exchange(dest, ptr, current, next_val, bbname_into_u32(next_block))
        });
    }

    pub fn expose_provenance(&mut self, dest: PlaceExpr, ptr: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            expose_provenance(dest, ptr, bbname_into_u32(next_block))
        });
    }

    pub fn with_exposed_provenance(&mut self, dest: PlaceExpr, addr: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            with_exposed_provenance(dest, addr, bbname_into_u32(next_block))
        });
    }

    pub fn lock_create(&mut self, ret: PlaceExpr) {
        self.finish_with_next_block(|next_block| lock_create(ret, bbname_into_u32(next_block)));
    }

    pub fn lock_acquire(&mut self, lock_id: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            lock_acquire(lock_id, bbname_into_u32(next_block))
        });
    }

    pub fn lock_release(&mut self, lock_id: ValueExpr) {
        self.finish_with_next_block(|next_block| {
            lock_release(lock_id, bbname_into_u32(next_block))
        });
    }

    pub fn start_unwind(&mut self, unwind_payload: ValueExpr, cleanup: BbName) {
        self.finish_block(start_unwind(unwind_payload, cleanup));
    }

    pub fn stop_unwind(&mut self, next_block: BbName) {
        self.finish_block(stop_unwind(next_block));
    }

    pub fn get_unwind_payload(&mut self, ret: PlaceExpr) {
        self.finish_with_next_block(|next_block| {
            get_unwind_payload(ret, bbname_into_u32(next_block))
        });
    }

    // terminators with 2 or more following blocks

    pub fn if_<F, G>(&mut self, condition: ValueExpr, then_branch: F, else_branch: G)
    where
        F: Fn(&mut Self),
        G: Fn(&mut Self),
    {
        self.switch_int(bool_to_int::<u8>(condition), &[(1, &then_branch)], else_branch);
    }

    pub fn switch_int<T, G>(
        &mut self,
        value: ValueExpr,
        cases: &[(T, &dyn Fn(&mut Self))],
        fallback: G,
    ) where
        T: Clone + Into<Int>,
        G: Fn(&mut Self),
    {
        // closures + blocks we we run the closures on
        let mut branches: Vec<(&dyn Fn(&mut Self), BbName)> = Vec::new();
        // branch map for switch terminator
        let mut branch_map: Map<Int, BbName> = Map::new();

        let block_kind = self.cur_block().kind;

        for (case, branch) in cases {
            let new_block = self.declare_block();
            branch_map.try_insert(case.clone().into(), new_block).unwrap();
            branches.push((branch, new_block));
        }

        let fallback_block = self.declare_block();
        let switch = Terminator::Switch { value, cases: branch_map, fallback: fallback_block };
        self.finish_block(switch);

        // None means that every branch finished on its own and we don't need a after_switch_block
        let mut after_switch_block: Option<BbName> = None;

        // Add the fallback block to the list of blocks to build.
        branches.push((&fallback, fallback_block));

        for (branch, block) in branches {
            self.set_cur_block(block, block_kind);
            branch(self);
            // If the current block not finished, jump to `after_switch_block`.
            if self.cur_block.is_some() {
                let jump_to_block = *after_switch_block.get_or_insert_with(|| self.declare_block());
                self.goto(jump_to_block);
            }
        }
        if let Some(after_switch_block) = after_switch_block {
            self.set_cur_block(after_switch_block, block_kind);
        }
    }

    pub fn while_<F: Fn(&mut Self)>(&mut self, condition: ValueExpr, body: F) {
        // goto new block such that condition sits alone in dedicated block
        let cond = self.declare_block();
        let block_kind = self.cur_block().kind;
        self.goto(cond);
        self.set_cur_block(cond, block_kind);

        self.if_(
            condition,
            |f| {
                body(f);
                if f.cur_block.is_some() {
                    f.goto(cond);
                }
            },
            |_| {},
        );
    }
}

pub fn goto(x: u32) -> Terminator {
    Terminator::Goto(BbName(Name::from_internal(x)))
}

pub fn if_(condition: ValueExpr, then_blk: u32, else_blk: u32) -> Terminator {
    Terminator::Switch {
        value: bool_to_int::<u8>(condition),
        cases: [(Int::from(1), BbName(Name::from_internal(then_blk)))].into_iter().collect(),
        fallback: BbName(Name::from_internal(else_blk)),
    }
}

pub fn switch_int<T: Clone + Into<Int>>(
    value: ValueExpr,
    cases: &[(T, u32)],
    fallback: u32,
) -> Terminator {
    Terminator::Switch {
        value,
        cases: cases
            .into_iter()
            .map(|(case, successor)| (case.clone().into(), BbName(Name::from_internal(*successor))))
            .collect(),
        fallback: BbName(Name::from_internal(fallback)),
    }
}

pub fn unreachable() -> Terminator {
    Terminator::Unreachable
}

pub fn call(f: u32, args: &[ArgumentExpr], ret: PlaceExpr, next: Option<u32>) -> Terminator {
    Terminator::Call {
        callee: fn_ptr_internal(f),
        calling_convention: CallingConvention::C, // FIXME do not hard-code the C calling convention
        arguments: args.iter().copied().collect(),
        ret,
        next_block: next.map(|x| BbName(Name::from_internal(x))),
        unwind_block: None,
    }
}

pub fn assume(val: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Assume,
        arguments: list![val],
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn print(arg: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::PrintStdout,
        arguments: list![arg],
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn eprint(arg: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::PrintStderr,
        arguments: list![arg],
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn allocate(size: ValueExpr, align: ValueExpr, ret_place: PlaceExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Allocate,
        arguments: list![size, align],
        ret: ret_place,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn deallocate(ptr: ValueExpr, size: ValueExpr, align: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Deallocate,
        arguments: list![ptr, size, align],
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn exit() -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Exit,
        arguments: list![],
        ret: unit_place(),
        next_block: None,
    }
}

pub fn abort() -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Abort,
        arguments: list![],
        ret: unit_place(),
        next_block: None,
    }
}

pub fn return_() -> Terminator {
    Terminator::Return
}

pub fn start_unwind(unwind_payload: ValueExpr, cleanup: BbName) -> Terminator {
    Terminator::StartUnwind { unwind_block: cleanup, unwind_payload }
}

pub fn stop_unwind(next_block: BbName) -> Terminator {
    Terminator::StopUnwind(next_block)
}

pub fn resume_unwind() -> Terminator {
    Terminator::ResumeUnwind
}

pub fn spawn(fn_ptr: ValueExpr, data_ptr: ValueExpr, ret: PlaceExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Spawn,
        arguments: list!(fn_ptr, data_ptr),
        ret,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn join(thread_id: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Join,
        arguments: list!(thread_id),
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn raw_eq(ret: PlaceExpr, left_ptr: ValueExpr, right_ptr: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::RawEq,
        arguments: list!(left_ptr, right_ptr),
        ret,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn atomic_store(ptr: ValueExpr, src: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::AtomicStore,
        arguments: list!(ptr, src),
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn atomic_load(dest: PlaceExpr, ptr: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::AtomicLoad,
        arguments: list!(ptr),
        ret: dest,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub enum FetchBinOp {
    Add,
    Sub,
}

pub fn atomic_fetch(
    binop: FetchBinOp,
    dest: PlaceExpr,
    ptr: ValueExpr,
    other: ValueExpr,
    next: u32,
) -> Terminator {
    let binop = match binop {
        FetchBinOp::Add => IntBinOp::Add,
        FetchBinOp::Sub => IntBinOp::Sub,
    };

    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::AtomicFetchAndOp(binop),
        arguments: list!(ptr, other),
        ret: dest,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn compare_exchange(
    dest: PlaceExpr,
    ptr: ValueExpr,
    current: ValueExpr,
    next_val: ValueExpr,
    next: u32,
) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::AtomicCompareExchange,
        arguments: list!(ptr, current, next_val),
        ret: dest,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn expose_provenance(dest: PlaceExpr, ptr: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::PointerExposeProvenance,
        arguments: list![ptr],
        ret: dest,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn with_exposed_provenance(dest: PlaceExpr, addr: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::PointerWithExposedProvenance,
        arguments: list![addr],
        ret: dest,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn lock_create(ret: PlaceExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Lock(IntrinsicLockOp::Create),
        arguments: list!(),
        ret: ret,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn lock_acquire(lock_id: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Lock(IntrinsicLockOp::Acquire),
        arguments: list!(lock_id),
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn lock_release(lock_id: ValueExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::Lock(IntrinsicLockOp::Release),
        arguments: list!(lock_id),
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(next))),
    }
}

pub fn get_unwind_payload(ret: PlaceExpr, next: u32) -> Terminator {
    Terminator::Intrinsic {
        intrinsic: IntrinsicOp::GetUnwindPayload,
        arguments: list!(),
        ret,
        next_block: Some(BbName(Name::from_internal(next))),
    }
}
