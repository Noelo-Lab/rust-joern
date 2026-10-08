int nested_return(int x) {
    try { if (x) { return x+1; } x++; }
    catch (...) { if (x) return x-1; x--; }
    return x;
}
int stmt_expr_return(int x) {
    try { x=({ if (x) return x+1; 2; }); }
    catch (...) { x=0; }
    return x;
}
