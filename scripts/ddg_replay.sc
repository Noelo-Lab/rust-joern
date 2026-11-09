// Original-Joern side of scripts/replay_ddg_projection.py. One run records,
// for every method FastParser.sc exports, the raw graph `dotDdg` reads (with
// each node's incoming REACHING_DEF edges in stored order) and that same run's
// `dotDdg`, so the projection can be replayed on exactly its input.
import io.shiftleft.codepropertygraph.generated.nodes.*
import java.io.{File, PrintWriter}
import scala.collection.mutable

def quote(text: String): String = {
  if (text == null) return "null"
  val out = new StringBuilder("\"")
  text.foreach {
    case '"'                           => out.append("\\\"")
    case '\\'                          => out.append("\\\\")
    case c if c < 0x20 || c >= 0x7f    => out.append(f"\\u${c.toInt}%04x")
    case c                             => out.append(c)
  }
  out.append('"').toString
}

def text(node: StoredNode, key: String): String =
  Option(node.propertiesMap.get(key)).map(v => quote(v.toString)).getOrElse("null")

@main def capture(target_dir: String, out: String) = {
  val cpg = importCode(target_dir)
  run.ossdataflow
  val writer = new PrintWriter(new File(out), "UTF-8")
  cpg.method
    .filter(m => m.lineNumber.isDefined && m.lineNumberEnd.isDefined && m.body.typeFullName != "<empty>")
    .foreach { method =>
      // The method's AST, then every outside node its REACHING_DEF edges lead
      // back to, each with the AST and ARGUMENT parents the projection reads.
      val own   = method.ast.l
      val nodes = mutable.LinkedHashSet.from[StoredNode](own)
      val queue = mutable.Queue.from[StoredNode](own)
      while (queue.nonEmpty) {
        queue.dequeue().inE("REACHING_DEF").map(_.src.asInstanceOf[StoredNode]).foreach { source =>
          if (nodes.add(source)) {
            queue.enqueue(source)
            (source.inE("ARGUMENT") ++ source.inE("AST")).foreach(e => nodes.add(e.src.asInstanceOf[StoredNode]))
          }
        }
      }
      val ids = nodes.map(_.id).toSet
      def code(n: StoredNode) = n match {
        case node: AstNode => quote(node.code)
        case _             => text(n, "CODE")
      }
      val nodeJson = nodes.toList.map { n =>
        s"""{"id":${n.id},"kind":${quote(n.label)},"code":${code(n)},"name":${text(n, "NAME")},""" +
          s""""canonical_name":${text(n, "CANONICAL_NAME")},"method_full_name":${text(n, "METHOD_FULL_NAME")},""" +
          s""""type_name":${text(n, "TYPE_FULL_NAME")},"line":${n.propertyOption[Int]("LINE_NUMBER").getOrElse(0)},""" +
          s""""column":${n.propertyOption[Int]("COLUMN_NUMBER").getOrElse(0)}}"""
      }
      def edge(kind: String, src: Long, dst: Long, label: String) =
        s"""[${quote(kind)},$src,$dst,$label]"""
      // AST and CFG in each source's stored out-order; ARGUMENT and
      // REACHING_DEF in each target's stored in-order, which is what
      // Expression.inCall and DdgGenerator.expand iterate.
      val structure = nodes.toList.flatMap { n =>
        List("AST", "CFG").flatMap(kind =>
          n.outE(kind).filter(e => ids.contains(e.dst.id)).map(e => edge(kind, n.id, e.dst.id, "null"))
        )
      }
      val arguments = nodes.toList.flatMap { n =>
        n.inE("ARGUMENT").filter(e => ids.contains(e.src.id)).map(e => edge("ARGUMENT", e.src.id, n.id, text(n, "ARGUMENT_INDEX")))
      }
      val reaching = nodes.toList.flatMap { n =>
        n.inE("REACHING_DEF").map { e =>
          val label = e.property match {
            case value: String => quote(value)
            case _             => "null"
          }
          edge("REACHING_DEF", e.src.id, n.id, label)
        }
      }
      val displayed =
        (List[StoredNode](method, method.methodReturn) ++ method.parameter.l ++ method.cfgNode.l).map(_.id)
      writer.println(
        s"""{"name":${quote(method.name)},"fullname":${quote(method.fullName)},"filename":${quote(method.filename)},""" +
          s""""signature":${quote(method.signature)},"start_line":${method.lineNumber.getOrElse(0)},""" +
          s""""end_line":${method.lineNumberEnd.getOrElse(0)},"own_nodes":${own.size},"nodes":[${nodeJson.mkString(",")}],""" +
          s""""edges":[${(structure ++ arguments ++ reaching).mkString(",")}],"order":[${displayed.mkString(",")}],""" +
          s""""dot_ddg":[${method.dotDdg.l.map(quote).mkString(",")}]}"""
      )
    }
  // Engine.methodsForCall resolves through CALL edges; `internal` and
  // `isNotStub` then classify the callee METHOD nodes of the whole CPG.
  // GNode.hashCode mixes in the Graph's identity hash, which is what orders
  // `solution.in.keys` in ReachingDefPass; it is recorded as provenance.
  val internal = cpg.method.internal.fullName.l.distinct
  val nonstub  = cpg.method.internal.isNotStub.fullName.l.distinct
  writer.println(
    s"""{"context":true,"graph_hash":${cpg.graph.hashCode()},"internal_methods":[${internal.map(quote).mkString(",")}],"nonstub_methods":[${nonstub
        .map(quote)
        .mkString(",")}]}"""
  )
  writer.close()
}
