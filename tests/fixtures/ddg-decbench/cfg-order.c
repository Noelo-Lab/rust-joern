int for_order(int *v, int n) {
  int i, s = 0;
  for (i = 0; i < n; i++) {
    if (v[i] < 0) continue;
    if (v[i] == 7) break;
    s += v[i];
    if (s > 100)
      s = 0;
  }
  return s;
}
int while_order(int *v, int n) {
  int s = 0;
  while (n > 0) {
    n--;
    while (v[n]) { v[n]--; if (v[n] == 3) s++; }
    if (s) { if (n) s--; } else if (v[0]) s++;
  }
  return s;
}
int do_order(int *v, int n) {
  int s = 0;
  do {
    if (v[n]) continue;
    for (;;) { if (s) break; s++; if (n) continue; }
    if (s == 9) s = 1;
  } while (--n);
  return s;
}
int unbraced_order(int n) {
  int s = 0;
  for (; n; ) if (n & 1) s++;
  while (n) if (s) n--;
  do if (n) s--; while (s);
  for (int k = 0; k < n; k++) for (int j = 0; j < k; j++) if (j) s += j;
  return s;
}
