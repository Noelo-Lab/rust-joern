int work(int);
int ellipsis_condition(int n) { work(n); if (...) work(n); work(n); return n; }
int after_ellipsis_condition(int n) { return work(n); }
int ellipsis_assignment(int n) { n = ...; work(n); return n; }
int after_ellipsis_assignment(int n) { return work(n); }
int fabricated_declaration(int n) { int <0x49114d[is_4]|Stack bp-0xa8, 1 B>; work(n); return n; }
int after_fabricated_declaration(int n) { return work(n); }
int fabricated_lvalue(int n) { *((char *)&<0x49114d[is_4]|Stack bp-0xa8, 1 B> + n) = 45; work(n); return n; }
int after_fabricated_lvalue(int n) { return work(n); }
int vex_pseudo_expression(int n) { n = Reinterpret(F64->I64, unsupported_<class 'pyvex.expr.GetI'>()); work(n); return n; }
int after_vex_pseudo_expression(int n) { return work(n); }
int dewolf_ssa_name(int n) { n = add_overflow(temp0_1#1, n); work(n); return n; }
int after_dewolf_ssa_name(int n) { return work(n); }
int angr_concat(int n) { n = _INSERT(n CONCAT 0, 0, n); work(n); return n; }
int after_angr_concat(int n) { return work(n); }
int reko_out_argument(int n) { n = other(n, out result); work(n); return n; }
int after_reko_out_argument(int n) { return work(n); }
int reko_width_operator(int n) { n = n /32 divisor; work(n); return n; }
int after_reko_width_operator(int n) { return work(n); }
int dewolf_array_type(int n) { unsigned long [18] * value; work(n); return n; }
int after_dewolf_array_type(int n) { return work(n); }
int ghidra_punctuated_name(int n) { n = PTR_s___><___&(:_00269e08; work(n); return n; }
int after_ghidra_punctuated_name(int n) { return work(n); }
