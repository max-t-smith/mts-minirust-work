use crate::*;

/// This helper implements unsizing coercion with a transmute, instead of a `ConstructWidePointer`.
///
/// It builds code to create a `&[T]` place from an `[T; known_len]` place.
fn ref_as_transmuted_slice<T: TypeConv + Freeze>(
    f: &mut FunctionBuilder,
    arr: PlaceExpr,
    known_len: u64,
) -> PlaceExpr {
    // construct fake wide ptr
    let arr_ref = addr_of(arr, <*const T>::get_type());
    let pair_ty = PtrType::Raw { meta_kind: PointerMetaKind::ElementCount }
        .as_wide_pair::<miniutil::DefaultTarget>()
        .expect("PtrType is wide");
    let fake_ptr = f.declare_local_with_ty(pair_ty);
    f.storage_live(fake_ptr);
    f.assign(field(fake_ptr, 0), arr_ref);
    f.assign(field(fake_ptr, 1), const_int(known_len));
    f.validate(fake_ptr, false); // Bad for ZST ?
    // transmute into slice ref
    let slice = f.declare_local::<&[T]>();
    f.storage_live(slice);
    f.assign(slice, transmute(load(fake_ptr), <&[T]>::get_type()));
    f.validate(slice, false);
    slice
}

/// Tests that slices can occur behind different pointer types
#[test]
fn wf_slice_ref() {
    let mut p = ProgramBuilder::new();

    let _f = {
        let mut f = p.declare_function();
        let _var = f.declare_local::<&[u32]>();
        let _ret = f.declare_ret::<&mut [u8]>();
        let _arg = f.declare_arg::<*const [[[u8; 3]; 2]]>();
        f.exit();
        p.finish_function(f)
    };

    let main = {
        let mut main = p.declare_function();
        main.exit();
        p.finish_function(main)
    };

    let p = p.finish_program(main);
    assert_stop::<BasicMem>(p);
}

/// Tests that an index operation is well formed
#[test]
fn wf_index() {
    let mut p = ProgramBuilder::new();

    let _f = {
        let mut f = p.declare_function();
        let slice = f.declare_arg::<&[u32]>();
        let var = f.declare_local::<u32>();
        f.storage_live(var);
        let elem_place = index(deref(load(slice), <[u32]>::get_type()), const_int(2));
        f.assign(elem_place, const_int(42_u32));
        f.assign(var, load(elem_place));
        f.exit();
        p.finish_function(f)
    };

    let main = {
        let mut main = p.declare_function();
        main.exit();
        p.finish_function(main)
    };

    let p = p.finish_program(main);
    assert_stop::<BasicMem>(p);
}

/// Asserts that the slice element type must be sized
#[test]
fn ill_slice_ref_unsized_elem() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        let var = f.declare_local_with_ty(<&[u32]>::get_type());
        f.storage_live(var);
        let slice_place = deref(load(var), slice_ty(slice_ty(<u32>::get_type())));
        f.validate(slice_place, false);
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    assert_ill_formed::<BasicMem>(p, "Type::Slice: unsized element type");
}

/// Asserts that locals must be sized
#[test]
fn ill_local() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        // ill formed:
        f.declare_local_with_ty(<[u32]>::get_type());
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    assert_ill_formed::<BasicMem>(p, "Function: unsized local variable");
}

/// Asserts loads at unsized types are ill-formed
#[test]
fn ill_load() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        let var = f.declare_local_with_ty(<&[u32]>::get_type());
        f.storage_live(var);
        let slice_place = deref(load(var), <[u32]>::get_type());
        // ill formed load: (also ill formed print, but need some way to use the valueexpr)
        f.print(load(slice_place));
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    assert_ill_formed::<BasicMem>(p, "ValueExpr::Load: unsized value type");
}

/// Asserts transmuts to unsized types are ill-formed
#[test]
fn ill_transmute() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        let arr = f.declare_local::<[u32; 1]>();
        f.storage_live(arr);
        f.assign(index(arr, const_int(0)), const_int(42_u32));
        // ill formed transmute:
        let slice = transmute(load(arr), <[u32]>::get_type());
        // (also ill formed print, but need some way to use the valueexpr)
        f.print(slice);
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    assert_ill_formed::<BasicMem>(p, "Cast::Transmute: unsized target type");
}

/// Tests that a wide pointer can be transmuted from a `(*T, usize)`.
#[test]
fn index_to_transmuted_slice() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        // Make array
        let arr = f.declare_local::<[u32; 3]>();
        f.storage_live(arr);
        f.assign(index(arr, const_int(0)), const_int(42_u32));
        f.assign(index(arr, const_int(1)), const_int(43_u32));
        f.assign(index(arr, const_int(2)), const_int(44_u32));
        let slice_ptr = ref_as_transmuted_slice::<u32>(&mut f, arr, 3);
        // Print slice[1]
        let loaded_val = load(index(deref(load(slice_ptr), <[u32]>::get_type()), const_int(1)));
        f.assume(eq(loaded_val, const_int(43_u32)));
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    assert_stop::<BasicMem>(p);
}

/// Corresponds to
/// ```rust
/// let x: [u32; 3] = [42, 43, 44];
/// let y = &x as &[u32];
/// let z = *unsafe { y.get_unchecked(1) };
/// assert!(z == 43);
/// ```
#[test]
fn index_to_slice() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        // Make array
        let arr = f.declare_local::<[u32; 3]>();
        let slice = f.declare_local::<&[u32]>();
        let elem = f.declare_local::<u32>();
        f.storage_live(arr);
        f.storage_live(slice);
        f.storage_live(elem);
        f.assign(
            arr,
            array(&[const_int(42_u32), const_int(43_u32), const_int(44_u32)], <u32>::get_type()),
        );
        f.assign(
            slice,
            construct_wide_pointer(
                addr_of(arr, <&[u32; 3]>::get_type()),
                const_int(3_usize),
                <&[u32]>::get_type(),
            ),
        );
        // Load and check slice[1]
        f.assign(elem, load(index(deref(load(slice), <[u32]>::get_type()), const_int(1))));
        f.assume(eq(load(elem), const_int(43_u32)));
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    dump_program(p);
    assert_stop::<BasicMem>(p);
}

/// Tests that indexing into a slice throws UB for invalid indices
#[test]
fn ub_invalid_index() {
    fn for_index(idx: isize) {
        let mut p = ProgramBuilder::new();
        let f = {
            let mut f = p.declare_function();
            // Make array
            let arr = f.declare_local::<[u32; 2]>();
            f.storage_live(arr);
            f.assign(index(arr, const_int(0)), const_int(42_u32));
            f.assign(index(arr, const_int(1)), const_int(43_u32));
            let slice_ptr = construct_wide_pointer(
                addr_of(arr, <&[u32; 2]>::get_type()),
                const_int(2_usize),
                <&[u32]>::get_type(),
            );
            // This should UB
            let loaded_val = load(index(deref(slice_ptr, <[u32]>::get_type()), const_int(idx)));
            f.print(loaded_val);
            f.exit();
            p.finish_function(f)
        };
        let p = p.finish_program(f);
        assert_ub::<BasicMem>(p, "access to out-of-bounds index");
    }

    for_index(-1);
    for_index(2);
}

/// The total size of a raw slice pointer can be larger than isize::MAX
#[test]
fn large_raw() {
    let mut p = ProgramBuilder::new();
    let f = {
        let mut f = p.declare_function();
        // Make array
        let arr = f.declare_local::<[[u32; 0x1_0000]; 1]>();
        let wide = f.declare_local::<*const [[u32; 0x1_0000]]>();
        f.storage_live(arr);
        f.storage_live(wide);
        let slice_ptr = construct_wide_pointer(
            addr_of(arr, <&[[u32; 0x1_0000]; 1]>::get_type()),
            const_int(0x2000_0000_0000_usize), // total size of isize::MAX + 1
            <*const [[u32; 0x1_0000]]>::get_type(),
        );
        // This should be fine
        f.assign(wide, slice_ptr);
        f.exit();
        p.finish_function(f)
    };
    let p = p.finish_program(f);
    assert_stop::<BasicMem>(p);
}

/// The total size of a safe slice pointer cannot be larger than isize::MAX
#[test]
fn ub_too_large_slice() {
    let mut p = ProgramBuilder::new();
    let f = {
        let mut f = p.declare_function();
        // Make array
        let arr = f.declare_local::<[[u32; 0x1_0000]; 1]>();
        let wide = f.declare_local::<&[[u32; 0x1_0000]]>();
        f.storage_live(arr);
        f.storage_live(wide);
        let slice_ptr = construct_wide_pointer(
            addr_of(arr, <&[[u32; 0x1_0000]; 1]>::get_type()),
            const_int(0x2000_0000_0000_usize), // total size of isize::MAX + 1
            <&[[u32; 0x1_0000]]>::get_type(),
        );
        // This should UB
        f.assign(wide, slice_ptr);
        f.exit();
        p.finish_function(f)
    };
    let p = p.finish_program(f);
    assert_ub::<BasicMem>(p, "Value::Ptr: total size exeeds isize::MAX");
}

/// ```rust
/// let x: [u32; 3] = [42, 43, 44];
/// let y = &x as &[u32];
/// assert!(y.len() == 3);
/// ```
#[test]
fn get_metadata_correct() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        // Make array
        let arr = f.declare_local::<[u32; 3]>();
        f.storage_live(arr);
        f.assign(
            arr,
            array(&[const_int(42_u32), const_int(43_u32), const_int(44_u32)], <u32>::get_type()),
        );
        // Construct a slice reference
        let slice = f.declare_local::<&[u32]>();
        f.storage_live(slice);
        f.assign(
            slice,
            construct_wide_pointer(
                addr_of(arr, <&[u32; 3]>::get_type()),
                const_int(3_usize),
                <&[u32]>::get_type(),
            ),
        );
        // Get the metadata again && assert it to be correct
        let loaded_len = get_metadata(load(slice));
        f.assume(eq(loaded_len, const_int(3_usize)));
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    dump_program(p);
    assert_stop::<BasicMem>(p);
}

/// Asserts the thin pointer of a slice points to the first element
#[test]
fn get_thin_pointer_is_first_elem() {
    let mut p = ProgramBuilder::new();

    let f = {
        let mut f = p.declare_function();
        // Make array
        let arr = f.declare_local::<[u32; 3]>();
        f.storage_live(arr);
        f.assign(index(arr, const_int(0)), const_int(42_u32));
        f.assign(index(arr, const_int(1)), const_int(43_u32));
        f.assign(index(arr, const_int(2)), const_int(44_u32));
        // Construct a slice reference
        let slice_ptr = construct_wide_pointer(
            addr_of(arr, <&[u32; 3]>::get_type()),
            const_int(3_usize),
            <&[u32]>::get_type(),
        );
        // Get the thin pointer & load an u32 at this address, should be the first element
        let loaded_start = get_thin_pointer(slice_ptr);
        f.assume(eq(load(deref(loaded_start, <u32>::get_type())), const_int(42_u32)));
        f.exit();
        p.finish_function(f)
    };

    let p = p.finish_program(f);
    assert_stop::<BasicMem>(p);
}
