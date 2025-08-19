use crate::build::*;

pub fn int_ty(signed: Signedness, size: Size) -> Type {
    Type::Int(IntType { signed, size })
}

pub fn bool_ty() -> Type {
    Type::Bool
}

pub fn ref_ty(pointee: PointeeInfo) -> Type {
    Type::Ptr(PtrType::Ref { mutbl: Mutability::Immutable, pointee })
}

/// Create an UnsafeCellStrategy with no UnsafeCell bytes from a LayoutStrategy.
pub fn from_frozen_layout(layout: LayoutStrategy) -> UnsafeCellStrategy {
    match layout {
        LayoutStrategy::Sized(..) => UnsafeCellStrategy::Sized { bytes: List::new() },
        LayoutStrategy::Slice(..) => UnsafeCellStrategy::Slice { element: List::new() },
        LayoutStrategy::TraitObject(..) => UnsafeCellStrategy::TraitObject { is_freeze: true },
        LayoutStrategy::Tuple { tail, .. } =>
            UnsafeCellStrategy::Tuple {
                head: List::new(),
                tail: GcCow::new(from_frozen_layout(tail.extract())),
            },
    }
}

/// Create a minirust reference type for a minirust type which implements default marker traits,
/// i.e. the type is `Unpin`, `Freeze` and is inhabited.
pub fn ref_ty_default_markers_for(ty: Type) -> Type {
    let layout = ty.layout::<DefaultTarget>();
    let unsafe_cells = from_frozen_layout(layout);

    ref_ty(PointeeInfo { layout, inhabited: true, unsafe_cells, unpin: true })
}

pub fn ref_mut_ty(pointee: PointeeInfo) -> Type {
    Type::Ptr(PtrType::Ref { mutbl: Mutability::Mutable, pointee })
}

/// Create a mutable minirust reference type for a minirust type which implements default marker traits,
/// i.e. the type is `Unpin`, `Freeze` and is inhabited.
pub fn ref_mut_ty_default_markers_for(ty: Type) -> Type {
    let layout = ty.layout::<DefaultTarget>();
    let unsafe_cells = from_frozen_layout(layout);

    ref_mut_ty(PointeeInfo { layout, inhabited: true, unsafe_cells, unpin: true })
}

pub fn box_ty(pointee: PointeeInfo) -> Type {
    Type::Ptr(PtrType::Box { pointee })
}

pub fn raw_ptr_ty(meta_kind: PointerMetaKind) -> Type {
    Type::Ptr(PtrType::Raw { meta_kind })
}

pub fn raw_void_ptr_ty() -> Type {
    raw_ptr_ty(PointerMetaKind::None)
}

pub fn tuple_ty(f: &[(Offset, Type)], size: Size, align: Align) -> Type {
    Type::Tuple {
        sized_fields: f.iter().copied().collect(),
        sized_head_layout: TupleHeadLayout { end: size, align, packed_align: None },
        unsized_field: GcCow::new(None),
    }
}

pub fn unsized_tuple_ty(
    fs: &[(Offset, Type)],
    unsized_ty: Type,
    end: Offset,
    align: Align,
    packed_align: Option<Align>,
) -> Type {
    Type::Tuple {
        sized_fields: fs.iter().copied().collect(),
        sized_head_layout: TupleHeadLayout { end, align, packed_align },
        unsized_field: GcCow::new(Some(unsized_ty)),
    }
}

pub fn union_ty(f: &[(Offset, Type)], size: Size, align: Align) -> Type {
    let chunks = list![(Size::ZERO, size)];
    Type::Union { fields: f.iter().copied().collect(), size, align, chunks }
}

pub fn array_ty(elem: Type, count: impl Into<Int>) -> Type {
    Type::Array { elem: GcCow::new(elem), count: count.into() }
}

pub fn slice_ty(elem: Type) -> Type {
    Type::Slice { elem: GcCow::new(elem) }
}

pub fn trait_object_ty(trait_name: TraitName) -> Type {
    Type::TraitObject(trait_name)
}

pub fn enum_variant(ty: Type, tagger: &[(Offset, (IntType, Int))]) -> Variant {
    Variant { ty, tagger: tagger.iter().copied().collect() }
}

pub fn enum_ty<DiscriminantTy: TypeConv + Into<Int> + Copy>(
    variants: &[(DiscriminantTy, Variant)],
    discriminator: Discriminator,
    size: Size,
    align: Align,
) -> Type {
    let Type::Int(discriminant_ty) = DiscriminantTy::get_type() else {
        panic!("Discriminant Type needs to be an integer type.");
    };
    Type::Enum {
        variants: variants.iter().copied().map(|(disc, variant)| (disc.into(), variant)).collect(),
        discriminator,
        discriminant_ty,
        size,
        align,
    }
}

pub fn discriminator_invalid() -> Discriminator {
    Discriminator::Invalid
}

pub fn discriminator_known(discriminant: impl Into<Int>) -> Discriminator {
    Discriminator::Known(discriminant.into())
}

/// Builds a branching discriminator on the type given by the generic which has to be an integer type.
pub fn discriminator_branch<T: ToInt + TypeConv + Copy>(
    offset: Offset,
    fallback: Discriminator,
    children: &[((T, T), Discriminator)],
) -> Discriminator {
    let Type::Int(value_type) = T::get_type() else { unreachable!() };
    Discriminator::Branch {
        offset,
        value_type,
        fallback: GcCow::new(fallback),
        children: children
            .into_iter()
            .copied()
            .map(|((start, end), disc)| ((start.into(), end.into()), disc))
            .collect(),
    }
}
