void g(void);
void comma_assertion(int x) {
    ((void)sizeof(x ? 1 : 0), __extension__ ({ if (x) ; else g(); }));
}
int comma_assertion_return(int x) {
    ((void)sizeof(x ? 1 : 0),
        __extension__ ({ if (x) ; else g(); }));
    return x;
}
int comma_assertion_if(int x) {
    if (x) ((void)sizeof(x ? 1 : 0), __extension__ ({ if (x) ; else g(); }));
    return x;
}
int comma_assertion_missing_return(int x) {
    if (x) ((void)sizeof(x ? 1 : 0),
        __extension__ ({ if (x) ; else g(); }));
}
