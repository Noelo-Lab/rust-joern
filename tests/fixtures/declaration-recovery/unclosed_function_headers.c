int before(int n) { return n; }
Unknown grouped_out(Unknown n, int (&out)
{ if(n) return n; return 0; }
int after_grouped_out(int n) { return n; }
int missing_close(int n
{ if(n) return n; return 0; }
int after_missing_close(int n) { return n; }
int double_group(int (&out, int (&other)
{ return 0; }
int after_double_group(int n) { return n; }
int valid_grouped(int (&out)) { return out; }
int after_valid(int n) { return n; }
