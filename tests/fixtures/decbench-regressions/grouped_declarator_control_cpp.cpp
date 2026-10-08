int g(int);
int while_scalar(int x) { while (g(x) < x) LODWORD(x) = g(x); return 0; }
int if_scalar(int x) { if (g(x) < x) LODWORD(x) = g(x); return 0; }
int if_else_scalar(int x) { if (g(x) < x) LODWORD(x) = g(x); else x = 2; return 0; }
int do_scalar(int x) { do LODWORD(x) = g(x); while (g(x) < x); return 0; }
int for_scalar(int x) { for (; g(x) < x; ++x) LODWORD(x) = g(x); return 0; }
int ordinary_block(int x) { LODWORD(x) = g(x); return 0; }
int while_block(int x) { while (g(x) < x) { LODWORD(x) = g(x); } return 0; }
