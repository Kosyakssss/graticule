use crate::grid::Area;

const FAR: i32 = 1 << 20;

fn any_area() -> Area {
    let area = Area::new(kani::any(), kani::any(), kani::any(), kani::any());
    kani::assume((-FAR..=FAR).contains(&area.col) && (-FAR..=FAR).contains(&area.row));
    kani::assume((0..=FAR).contains(&area.cols) && (0..=FAR).contains(&area.rows));
    area
}

fn partitions(whole: Area, cut: Area, rest: Area) -> bool {
    let stacked = cut.col == whole.col
        && rest.col == whole.col
        && cut.cols == whole.cols
        && rest.cols == whole.cols
        && cut.rows + rest.rows == whole.rows
        && (cut.bottom() == rest.row || rest.bottom() == cut.row);
    let side_by_side = cut.row == whole.row
        && rest.row == whole.row
        && cut.rows == whole.rows
        && rest.rows == whole.rows
        && cut.cols + rest.cols == whole.cols
        && (cut.right() == rest.col || rest.right() == cut.col);
    cut.cols >= 0 && cut.rows >= 0 && rest.cols >= 0 && rest.rows >= 0 && whole.holds(cut) && whole.holds(rest) && (stacked || side_by_side)
}

#[kani::proof]
fn taking_from_any_side_splits_the_area_in_two() {
    let whole = any_area();
    let n: i32 = kani::any();
    kani::assume((-2 * FAR..=2 * FAR).contains(&n));
    let side: u8 = kani::any();
    let mut rest = whole;
    let cut = match side % 4 {
        0 => rest.take_top(n),
        1 => rest.take_bottom(n),
        2 => rest.take_left(n),
        _ => rest.take_right(n),
    };
    assert!(partitions(whole, cut, rest));
}

#[kani::proof]
#[kani::solver(kissat)]
fn columns_fill_the_room_in_order_without_overlapping() {
    let whole = any_area();
    kani::assume(whole.cols <= 512);
    let weights: [i32; 3] = [kani::any(), kani::any(), kani::any()];
    kani::assume(weights.iter().all(|w| (0..=16).contains(w)));
    let gap: i32 = kani::any();
    kani::assume((0..=8).contains(&gap) && whole.cols >= gap * 2);
    let out = whole.columns(weights, gap);
    let mut used = 0;
    for (i, column) in out.iter().enumerate() {
        assert!(whole.holds(*column));
        assert!(column.cols >= 0);
        if i > 0 {
            assert!(column.col == out[i - 1].right() + gap);
        }
        used += column.cols;
    }
    assert!(out[0].col == whole.col);
    assert!(used + gap * 2 == whole.cols);
}

#[kani::proof]
fn a_cell_in_a_held_area_is_in_the_area_that_holds_it() {
    let outer = any_area();
    let inner = any_area();
    let (col, row): (i32, i32) = (kani::any(), kani::any());
    if outer.holds(inner) && inner.contains(col, row) {
        assert!(outer.contains(col, row));
    }
}
