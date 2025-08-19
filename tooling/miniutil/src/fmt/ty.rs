use super::*;

pub(super) fn fmt_type(t: Type, comptypes: &mut Vec<CompType>) -> FmtExpr {
    match t {
        Type::Int(int_ty) => FmtExpr::Atomic(fmt_int_type(int_ty)),
        Type::Ptr(ptr_ty) => fmt_ptr_type(ptr_ty),
        Type::Bool => FmtExpr::Atomic(format!("bool")),
        Type::Tuple { .. } | Type::Union { .. } | Type::Enum { .. } => {
            let comp_ty = CompType(t);
            let comptype_index = get_comptype_index(comp_ty, comptypes);
            FmtExpr::Atomic(fmt_comptype_index(comptype_index))
        }
        Type::Array { elem, count } => {
            let elem = fmt_type(elem.extract(), comptypes).to_string();
            FmtExpr::Atomic(format!("[{elem}; {count}]"))
        }
        Type::Slice { elem } => {
            let elem = fmt_type(elem.extract(), comptypes).to_string();
            FmtExpr::Atomic(format!("[{elem}]"))
        }
        Type::TraitObject(trait_name) =>
            FmtExpr::Atomic(format!("dyn {}", fmt_trait_name(trait_name))),
    }
}

pub(super) fn fmt_int_type(int_ty: IntType) -> String {
    let signed = match int_ty.signed {
        Signed => "i",
        Unsigned => "u",
    };
    let bits = int_ty.size.bits();

    format!("{signed}{bits}")
}

pub(super) fn fmt_ptr_type(ptr_ty: PtrType) -> FmtExpr {
    match ptr_ty {
        PtrType::Ref { mutbl: Mutability::Mutable, pointee } => {
            let pointee_info_str = fmt_pointee_info(pointee);
            FmtExpr::NonAtomic(format!("&mut {pointee_info_str}"))
        }
        PtrType::Ref { mutbl: Mutability::Immutable, pointee } => {
            let pointee_info_str = fmt_pointee_info(pointee);
            FmtExpr::NonAtomic(format!("&{pointee_info_str}"))
        }
        PtrType::Box { pointee } => {
            let pointee_info_str = fmt_pointee_info(pointee);
            FmtExpr::Atomic(format!("Box<{pointee_info_str}>"))
        }
        PtrType::Raw { meta_kind } => {
            let meta_kind_str = fmt_meta_kind(meta_kind);
            FmtExpr::NonAtomic(format!("*raw({meta_kind_str})"))
        }
        PtrType::FnPtr => FmtExpr::Atomic(format!("fn()")),
        PtrType::VTablePtr(_) => FmtExpr::Atomic("{vtable}".into()),
    }
}

fn fmt_meta_kind(kind: PointerMetaKind) -> String {
    match kind {
        PointerMetaKind::None => "thin".into(),
        PointerMetaKind::ElementCount => "meta=len".into(),
        PointerMetaKind::VTablePointer(trait_name) =>
            format!("meta=vtable<{}>", fmt_trait_name(trait_name)),
    }
}

fn fmt_layout_strategy(layout: LayoutStrategy) -> String {
    match layout {
        LayoutStrategy::Sized(size, align) =>
            format!("size={}, align={}", size.bytes(), align.bytes()),
        LayoutStrategy::Slice(size, align) =>
            format!("size={}*len, align={}", size.bytes(), align.bytes()),
        LayoutStrategy::TraitObject(..) => "size,align={unknown}".into(),
        LayoutStrategy::Tuple { head, tail } => {
            let packed_str = if let Some(packed) = head.packed_align {
                format!(", packed={}", packed.bytes())
            } else {
                "".into()
            };
            format!(
                "head=(end={}, align={}){}, tail=({})",
                head.end.bytes(),
                head.align.bytes(),
                packed_str,
                fmt_layout_strategy(tail.extract())
            )
        }
    }
}

fn fmt_pointee_info(pointee: PointeeInfo) -> String {
    let layout_str = fmt_layout_strategy(pointee.layout);
    let uninhab_str = match pointee.inhabited {
        true => "",
        false => ", uninhabited",
    };
    let freeze_str = match pointee.unsafe_cells.is_freeze_outside() {
        true => "",
        false => ", !Freeze",
    };
    let pin_str = match pointee.unpin {
        true => "",
        false => ", !Unpin",
    };
    let meta_str = fmt_meta_kind(pointee.layout.meta_kind());
    format!("pointee_info({meta_str}, {layout_str}{uninhab_str}{freeze_str}{pin_str})")
}

/////////////////////
// composite types
/////////////////////

// A "composite" type is a union or tuple.
// Composite types will be printed separately above the functions, as inlining them would be hard to read.
// During formatting, the list of composite types we encounter will be stored in `comptypes`.
#[derive(PartialEq, Eq, Clone, Copy)]
pub(super) struct CompType(pub(super) Type);

// An index into `comptypes`.
pub(super) struct CompTypeIndex {
    idx: usize,
}

// Gives the index of `ty` within `comptypes`.
// This adds `ty` to `comptypes` if it has been missing.
fn get_comptype_index(ty: CompType, comptypes: &mut Vec<CompType>) -> CompTypeIndex {
    let idx = match comptypes.iter().position(|x| *x == ty) {
        Some(i) => i,
        None => {
            let n = comptypes.len();
            comptypes.push(ty);
            n
        }
    };

    CompTypeIndex { idx }
}

fn fmt_comptype_index(comptype_index: CompTypeIndex) -> String {
    let id = comptype_index.idx;
    format!("T{id}")
}

// Formats all composite types.
pub(super) fn fmt_comptypes(mut comptypes: Vec<CompType>) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < comptypes.len() {
        let c = comptypes[i];
        let comptype_index = CompTypeIndex { idx: i };

        // A call to `fmt_comptype` might find new `CompTypes` and push them to `comptypes`.
        // Hence, we cannot use an iterator here.
        let s = &*fmt_comptype(comptype_index, c, &mut comptypes);

        out += s;

        i += 1;
    }

    out
}

fn fmt_comptype(i: CompTypeIndex, t: CompType, comptypes: &mut Vec<CompType>) -> String {
    let keyword = match t.0 {
        Type::Tuple { .. } => "tuple",
        Type::Union { .. } => "union",
        Type::Enum { .. } => "enum",
        _ => panic!("not a supported composite type!"),
    };
    let ct = fmt_comptype_index(i).to_string();
    let layout = fmt_layout_strategy(t.0.layout::<DefaultTarget>());
    let mut s = format!("{keyword} {ct} ({layout}) {{\n");
    match t.0 {
        Type::Tuple { sized_fields, unsized_field, .. } => {
            s += &fmt_comptype_fields(sized_fields, comptypes);
            if let Some(unsized_ty) = unsized_field.extract() {
                let ty = fmt_type(unsized_ty, comptypes).to_string();
                s += &format!("  tail: {ty},\n");
            }
        }
        Type::Union { fields, chunks, .. } => {
            s += &fmt_comptype_fields(fields, comptypes);
            s += &fmt_comptype_chunks(chunks);
        }
        Type::Enum { variants, discriminant_ty, .. } => {
            let discr = fmt_int_type(discriminant_ty);
            s += &format!("  Discriminant: {discr}\n");
            variants.iter().for_each(|(discriminant, v)| {
                let typ = fmt_type(v.ty, comptypes).to_string();
                s += &format!("  Variant {discriminant}: {typ}\n");
            });
        }
        _ => panic!("not a supported composite type!"),
    };
    s += "}\n\n";
    s
}

fn fmt_comptype_fields(fields: Fields, comptypes: &mut Vec<CompType>) -> String {
    let mut s = String::new();
    for (offset, f) in fields {
        let offset = offset.bytes();
        let ty = fmt_type(f, comptypes).to_string();
        s += &format!("  at byte {offset}: {ty},\n");
    }
    s
}

fn fmt_comptype_chunks(chunks: List<(Offset, Size)>) -> String {
    let mut s = String::new();
    for (offset, size) in chunks {
        let offset = offset.bytes();
        let size = size.bytes();
        s += &format!("  chunk(at={offset}, size={size}),\n");
    }
    s
}
