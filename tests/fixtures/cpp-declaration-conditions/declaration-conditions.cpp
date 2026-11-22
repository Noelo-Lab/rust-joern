int next();
void use(int value);

int classify() {
  switch (int kind = next()) {
  case 0:
    return 1;
  default:
    use(kind);
    return 2;
  }
}

int drain() {
  int total = 0;
  while (int item = next()) {
    total += item;
    use(total);
  }
  return total;
}

int choose() {
  if (int value = next()) {
    use(value);
    return value;
  }
  return 0;
}

int pick() {
  switch (next()) {
  case 1:
    return 3;
  }
  while (next()) {
    use(4);
  }
  return 5;
}
