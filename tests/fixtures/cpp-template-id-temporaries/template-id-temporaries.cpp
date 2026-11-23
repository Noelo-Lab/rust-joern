int ordered_return(int lo, int hi) {
  return std::less_equal<int>()(lo, hi);
}

int pointer_window(const char *base, const void *mark, unsigned long span) {
  int inside = 0;
  if (span > 0 && std::less_equal<const void *>()(base + span, mark)) {
    inside = 1;
  }
  return inside;
}

int disjoint(const char *a, const char *b, unsigned long n) {
  bool apart = std::less_equal<const void *>()(a + n, b) ||
               std::less_equal<const void *>()(b + n, a);
  while (!apart && n > 0) {
    n = n / 2;
    apart = std::greater<std::pair<int, int>>()({1, 2}, {1, static_cast<int>(n)});
  }
  return apart ? 1 : 0;
}

int comparison_control(int a, int b, int c) {
  int r = 0;
  if (a < b > (c)) {
    r = a;
  }
  return r + (a < b) + (b > (c));
}
