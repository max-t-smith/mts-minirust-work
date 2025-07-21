use crate::*;

/// This test checks that using `goto` to jump to a block of a different kind results in an ill-formed program.
#[test]
fn goto_wrong_blockkind() {
    let bb0 = block!(goto(1));
    let bb1 = block(&[], exit(), BbKind::Cleanup);
    let f = function(Ret::No, 0, &[], &[bb0, bb1]);
    let p = program(&[f]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block has the wrong block kind");
}

/// This test checks that using `switch` to jump to a block of a different kind results in an ill-formed program.
#[test]
fn switch_wrong_blockkind() {
    let bb0 = block!(switch_int(const_int(0), &[(0u8, 1), (1u8, 1), (7u8, 2)], 1));
    let bb1 = block!(exit());
    let bb2 = block(&[], exit(), BbKind::Cleanup);
    let f = function(Ret::No, 0, &[], &[bb0, bb1, bb2]);
    let p = program(&[f]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block has the wrong block kind");
}

/// This test checks that using `switch` to jump to a block of a different kind in the fallback results in an ill-formed program.
#[test]
fn switch_wrong_blockkind_fallback() {
    let bb0 = block!(switch_int(const_int(0), &[(0u8, 1), (1u8, 1), (7u8, 1)], 2));
    let bb1 = block!(exit());
    let bb2 = block(&[], exit(), BbKind::Cleanup);
    let f = function(Ret::No, 0, &[], &[bb0, bb1, bb2]);
    let p = program(&[f]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block has the wrong block kind");
}

/// This test checks that using an intrinsic to jump to a block of a different kind results in an ill-formed program.
#[test]
fn intrinsic_wrong_blockkind() {
    let bb0 = block!(print(const_int(0), 1));
    let bb1 = block(&[], exit(), BbKind::Cleanup);
    let f = function(Ret::No, 0, &[], &[bb0, bb1]);
    let p = program(&[f]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block has the wrong block kind");
}

/// This test checks that a `call` returning to a block of a different kind results in an ill-formed program.
#[test]
fn call_nextblock_wrong_kind() {
    let bb0 = block!(Terminator::Call {
        callee: fn_ptr_internal(1),
        calling_convention: CallingConvention::C,
        arguments: list![],
        ret: unit_place(),
        next_block: Some(BbName(Name::from_internal(1))),
        unwind_block: None,
    });
    let bb1 = block(&[], exit(), BbKind::Terminate);
    let f0 = function(Ret::No, 0, &[], &[bb0, bb1]);

    let f1 = function(Ret::No, 0, &[], &[block!(return_())]);
    let p = program(&[f0, f1]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block has the wrong block kind");
}
/// This test checks that a `call`, where the unwind block has the wrong block kind, results in an ill-formed program.
#[test]
fn call_unwindblock_wrong_kind() {
    let bb0 = block!(Terminator::Call {
        callee: fn_ptr_internal(1),
        calling_convention: CallingConvention::C,
        arguments: list![],
        ret: unit_place(),
        next_block: None,
        unwind_block: Some(BbName(Name::from_internal(1))),
    });
    let bb1 = block!(exit());
    let f0 = function(Ret::No, 0, &[], &[bb0, bb1]);

    let f1 = function(Ret::No, 0, &[], &[block!(return_())]);
    let p = program(&[f0, f1]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: unwind block has the wrong block kind");
}

/// This test checks that using `StartUnwind` to jump to a regular block results in an ill-formed program.
#[test]
fn start_unwind_wrong_nextblock() {
    let bb0 = block!(start_unwind(unit_ptr(), BbName(Name::from_internal(1))));
    let bb1 = block!(exit());
    let f = function(Ret::No, 0, &[], &[bb0, bb1]);
    let p = program(&[f]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: unwind block has the wrong block kind");
}

/// This test uses `Return` in a cleanup block, which results in an ill-formed program.
#[test]
fn return_in_cleanup() {
    let mut p = ProgramBuilder::new();
    let f = {
        let mut f = p.declare_function();
        let c = f.cleanup_block(|f| f.return_());
        f.start_unwind(unit_ptr(), c);
        p.finish_function(f)
    };

    let main_fn = {
        let mut main_fn = p.declare_function();
        let c = main_fn.cleanup_block(|f| {
            f.abort();
        });
        main_fn.call(unit_place(), fn_ptr(f), &[], c);
        main_fn.exit();
        p.finish_function(main_fn)
    };

    let p = p.finish_program(main_fn);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator::Return has to be called in a regular block");
}

/// This test uses `StartUnwind` in a cleanup block, which results in an ill-formed program.
#[test]
fn start_unwind_in_cleanup() {
    let mut p = ProgramBuilder::new();
    let f = {
        let mut f = p.declare_function();
        let outer_cleanup = f.cleanup_block(|f| {
            let inner_cleanup = f.cleanup_block(|f| f.exit());
            f.start_unwind(unit_ptr(), inner_cleanup);
        });
        f.start_unwind(unit_ptr(), outer_cleanup);
        p.finish_function(f)
    };
    let p = p.finish_program(f);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator::StartUnwind has to be called in a regular block");
}

/// This test uses `ResumeUnwind` in a regular block, which results in an ill-formed program.
#[test]
fn resume_in_regular_block() {
    let mut p = ProgramBuilder::new();
    let f = {
        let mut f = p.declare_function();
        f.resume_unwind();
        p.finish_function(f)
    };
    let p = p.finish_program(f);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator::ResumeUnwind: has to be called in cleanup block");
}

/// Return some basic function.
fn other_f() -> Function {
    let locals = [<()>::get_type(); 2];
    let b0 = block!(exit());
    function(Ret::Yes, 1, &locals, &[b0])
}

/// In this test the next block of the `call` does not exist, which results in an ill-formed program.
#[test]
fn call_next_block_non_exist() {
    let locals = [<()>::get_type()];

    let b0 = block!(
        storage_live(0),
        Terminator::Call {
            callee: fn_ptr_internal(1),
            calling_convention: CallingConvention::C,
            arguments: list![by_value(unit())],
            ret: local(0),
            next_block: Some(BbName(Name::from_internal(2))),
            unwind_block: Some(BbName(Name::from_internal(1))),
        }
    );
    let b1 = block!(exit());

    let f = function(Ret::No, 0, &locals, &[b0, b1]);
    let p = program(&[f, other_f()]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block does not exist");
}

/// In this test the unwind block of the `call` does not exist, which results in an ill-formed program.
#[test]
fn unwind_block_non_exist() {
    let locals = [<()>::get_type()];

    let b0 = block!(
        storage_live(0),
        Terminator::Call {
            callee: fn_ptr_internal(1),
            calling_convention: CallingConvention::C,
            arguments: list![by_value(unit())],
            ret: local(0),
            next_block: Some(BbName(Name::from_internal(1))),
            unwind_block: Some(BbName(Name::from_internal(2))),
        }
    );
    let b1 = block!(exit());

    let f = function(Ret::No, 0, &locals, &[b0, b1]);
    let p = program(&[f, other_f()]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: unwind block does not exist");
}

/// In this test, the call in the catch block has an unwinding control-flow edge, which results in an ill-formed program.
#[test]
fn unwind_in_catch_block() {
    let mut p = ProgramBuilder::new();

    let panic_fn = {
        let mut f = p.declare_function();
        f.print(const_int(2));
        let cleanup = f.cleanup_block(|f| f.resume_unwind());
        f.start_unwind(unit_ptr(), cleanup);
        p.finish_function(f)
    };

    let main_fn = {
        let mut f = p.declare_function();

        let cont = f.declare_block();

        let cleanup_block = f.cleanup_block(|f| f.abort());
        let catch_block = f.catch_block(|f| {
            f.call(unit_place(), fn_ptr(panic_fn), &[], cleanup_block);
            f.stop_unwind(cont);
        });

        f.call(unit_place(), fn_ptr(panic_fn), &[], catch_block);
        f.goto(cont);
        f.set_cur_block(cont, BbKind::Regular);
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(main_fn);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: unwinding is not allowed in a catch block");
}

/// In this test there is a `goTo`, that jumps from a cleanup to a catch block, which results in an ill-formed program.
#[test]
fn goto_from_cleanup_to_catch() {
    let locals = [<()>::get_type()];

    let b0 = block!(storage_live(0), start_unwind(unit_ptr(), BbName(Name::from_internal(1))));
    let b1 = block(&[], Terminator::Goto(BbName(Name::from_internal(2))), BbKind::Cleanup);
    let b2 = block(&[], exit(), BbKind::Catch);

    let f = function(Ret::No, 0, &locals, &[b0, b1, b2]);
    let p = program(&[f, other_f()]);
    dump_program(p);
    assert_ill_formed::<BasicMem>(p, "Terminator: next block has the wrong block kind");
}
