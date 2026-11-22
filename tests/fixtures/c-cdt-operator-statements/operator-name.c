int operator(int n)
{
  if (n > 0)
    return n - 1;
  return n;
}

int apply(int n)
{
  int total;
  total = operator(n);
  while (total)
    total = operator(total);
  return total;
}
