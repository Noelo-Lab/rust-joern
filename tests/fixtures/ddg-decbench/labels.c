int continued(double v2, double v3, int mode) {
  int result = 0;
  if (v2 <= 0.1
      || v2 >= 0.89999998
      || v3 > v2)
    result = 1;
  result = result
           | mode
           | 010;
  return result;
}
