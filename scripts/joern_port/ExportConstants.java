import java.lang.reflect.*;
import java.util.*;
public class ExportConstants {
  public static void main(String[] args) throws Exception {
    for (String name : args) {
      Class<?> cls = Class.forName(name);
      Field[] fields = cls.getFields();
      Arrays.sort(fields, Comparator.comparing(Field::getName));
      for (Field field : fields) {
        if (Modifier.isStatic(field.getModifiers()) && (field.getType() == String.class || field.getType() == int.class)) {
          System.out.println(name + "\t" + field.getName() + "\t" + field.get(null));
        }
      }
    }
  }
}
