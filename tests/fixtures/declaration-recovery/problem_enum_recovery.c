int before(int n) { return n; }
enum { 2 = 2 };
int after_numeric(int);
int kept_numeric(int n) { return n; }
enum { (-10) = -10 };
int after_grouped(int);
int kept_grouped(int n) { return n; }
enum Valid { ONE=1, TWO=2 };
int after_valid(int);
int kept_valid(int n) { return n; }
