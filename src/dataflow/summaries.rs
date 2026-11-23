//! Semantic summaries of library calls and access paths.

use super::*;

pub(super) fn exact_path(a: &[Access], b: &[Access]) -> bool {
    let head = |path: &[Access]| {
        path.iter()
            .rposition(|e| {
                !matches!(
                    e,
                    Access::Address | Access::Shift(_) | Access::VariableShift
                )
            })
            .map(|i| i + 1)
            .unwrap_or(0)
    };
    let (ah, bh) = (head(a), head(b));
    if ah != bh
        || a[..ah] != b[..bh]
        || a[..ah]
            .iter()
            .any(|e| matches!(e, Access::Variable | Access::VariableShift))
    {
        return false;
    }
    for (a, b) in a.iter().zip(b).skip(ah) {
        if matches!(
            (a, b),
            (
                Access::VariableShift,
                Access::Shift(_) | Access::VariableShift
            ) | (Access::Shift(_), Access::VariableShift)
        ) {
            return false;
        }
        if a != b {
            break;
        }
    }
    !b[bh..].contains(&Access::VariableShift)
}

pub(crate) fn semantics(name: &str) -> Option<Semantic> {
    let flows: &[(i32, i32)] = match name {
        "<operator>.addition"
        | "<operator>.cast"
        | "<operator>.elvis"
        | "calloc"
        | "difftime"
        | "difftime64"
        | "fdopen" => &[(1, -1), (2, -1)],
        "<operator>.assignment" => &[(2, 1), (2, -1)],
        "<operator>.assignmentAnd"
        | "<operator>.assignmentArithmeticShiftRight"
        | "<operator>.assignmentDivision"
        | "<operator>.assignmentExponentiation"
        | "<operator>.assignmentLogicalShiftRight"
        | "<operator>.assignmentMinus"
        | "<operator>.assignmentModulo"
        | "<operator>.assignmentMultiplication"
        | "<operator>.assignmentOr"
        | "<operator>.assignmentPlus"
        | "<operator>.assignmentShiftLeft"
        | "<operator>.assignmentXor" => &[(2, 1), (1, 1), (2, -1)],
        "<operators>.assignmentAnd"
        | "<operators>.assignmentArithmeticShiftRight"
        | "<operators>.assignmentModulo"
        | "<operators>.assignmentOr"
        | "<operators>.assignmentShiftLeft"
        | "<operators>.assignmentXor" => &[(2, 1), (1, 1), (2, -1)],
        "<operator>.addressOf"
        | "<operator>.computedMemberAccess"
        | "<operator>.notNullAssert"
        | "<operator>.fieldAccess"
        | "<operator>.getElementPtr"
        | "<operator>.indexAccess"
        | "<operator>.indirectComputedMemberAccess"
        | "<operator>.indirectFieldAccess"
        | "<operator>.indirectMemberAccess"
        | "<operator>.indirection"
        | "<operator>.memberAccess"
        | "<operator>.pointerShift"
        | "ctime"
        | "ctime64"
        | "ctime_r"
        | "ctime64_r"
        | "exp"
        | "fabs" => &[(1, -1)],
        "<operator>.indirectIndexAccess" => &[(1, -1), (2, 1)],
        "<operator>.conditional" => &[(2, -1), (3, -1)],
        "<operator>.incBy" => &[(1, 1), (2, 1), (3, 1), (4, 1)],
        "<operator>.postDecrement"
        | "<operator>.postIncrement"
        | "<operator>.preDecrement"
        | "<operator>.preIncrement"
        | "abs"
        | "asctime"
        | "asctime_r"
        | "atof"
        | "atoi"
        | "atol"
        | "fclose"
        | "feof"
        | "ferror"
        | "fflush"
        | "fgetc"
        | "strlen" => &[(1, 1), (1, -1)],
        "<operator>.sizeOf" | "abort" | "clock" => &[],
        "ceil" | "exit" | "free" | "getc" => &[(1, 1)],
        "fwrite" => &[(1, 1), (1, -1), (2, -1), (3, -1), (4, -1)],
        "scanf" => &[(2, 2)],
        "strcmp" => &[(1, 1), (1, -1), (2, 2), (2, -1)],
        "strncpy" => &[(1, 1), (2, 2), (3, 3), (1, -1), (2, -1)],
        "strncat" => &[(1, 1), (1, -1), (2, 2), (2, -1)],
        "<operator>.modulo" | "<operator>.arrayInitializer" | "div" => {
            return Some(Semantic {
                flows: &[],
                pass: true,
            });
        }
        _ => return None,
    };
    Some(Semantic { flows, pass: false })
}
