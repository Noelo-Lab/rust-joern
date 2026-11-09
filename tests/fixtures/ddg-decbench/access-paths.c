int copy_down(char *v4, char *v5, char *v1, unsigned v3, char *lim) {
  v5 = &v4[v3];
  if (lim < v5) {
    do {
      v1 = &v5[-1];
      v5 = &v5[-1];
      v4 = &v4[-1];
      *v4 = *v1;
    } while (lim < v5);
  }
  *lim = v4[-1];
  return v4[-1];
}
int literal_shift(int *p) {
  p[1] = p[0];
  p[1] = p[1] + 1;
  return p[1];
}
int bool_call(int v13, int v1) {
  int r;
  r = (bool)(v13 != 0 & v1);
  if ((bool)(r))
    r = (bool)v1;
  return r;
}
