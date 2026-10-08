int external_body(int x, int y);
int caller(int x, int y) { external_body(x, y); return x + y; }
