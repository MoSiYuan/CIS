const { Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell, 
        Header, Footer, AlignmentType, LevelFormat, HeadingLevel, BorderStyle, 
        WidthType, ShadingType, VerticalAlign, PageNumber, PageBreak, TableOfContents } = require('docx');
const fs = require('fs');

// Color scheme - Midnight Code
const colors = {
  primary: "020617",
  body: "1E293B",
  secondary: "64748B",
  accent: "94A3B8",
  tableBg: "F8FAFC"
};

const tableBorder = { style: BorderStyle.SINGLE, size: 12, color: colors.primary };
const cellBorders = { top: tableBorder, bottom: tableBorder, left: { style: BorderStyle.NONE }, right: { style: BorderStyle.NONE } };

const doc = new Document({
  styles: {
    default: { document: { run: { font: "SimSun", size: 24 } } },
    paragraphStyles: [
      { id: "Title", name: "Title", basedOn: "Normal",
        run: { size: 56, bold: true, color: colors.primary, font: "SimHei" },
        paragraph: { spacing: { before: 240, after: 120 }, alignment: AlignmentType.CENTER } },
      { id: "Heading1", name: "Heading 1", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 36, bold: true, color: colors.primary, font: "SimHei" },
        paragraph: { spacing: { before: 400, after: 200 }, outlineLevel: 0 } },
      { id: "Heading2", name: "Heading 2", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 28, bold: true, color: colors.body, font: "SimHei" },
        paragraph: { spacing: { before: 300, after: 150 }, outlineLevel: 1 } },
      { id: "Heading3", name: "Heading 3", basedOn: "Normal", next: "Normal", quickFormat: true,
        run: { size: 24, bold: true, color: colors.secondary, font: "SimHei" },
        paragraph: { spacing: { before: 200, after: 100 }, outlineLevel: 2 } }
    ]
  },
  numbering: {
    config: [
      { reference: "bullet-list",
        levels: [{ level: 0, format: LevelFormat.BULLET, text: "\u2022", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] },
      { reference: "numbered-1",
        levels: [{ level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] },
      { reference: "numbered-2",
        levels: [{ level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] },
      { reference: "numbered-3",
        levels: [{ level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] },
      { reference: "numbered-4",
        levels: [{ level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] },
      { reference: "numbered-5",
        levels: [{ level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] },
      { reference: "numbered-6",
        levels: [{ level: 0, format: LevelFormat.DECIMAL, text: "%1.", alignment: AlignmentType.LEFT,
          style: { paragraph: { indent: { left: 720, hanging: 360 } } } }] }
    ]
  },
  sections: [{
    properties: {
      page: { margin: { top: 1800, right: 1440, bottom: 1440, left: 1440 } }
    },
    headers: {
      default: new Header({ children: [new Paragraph({ 
        alignment: AlignmentType.RIGHT,
        children: [new TextRun({ text: "CIS \u4EE3\u7801\u5BA1\u67E5\u62A5\u544A", font: "SimHei", size: 20, color: colors.secondary })]
      })] })
    },
    footers: {
      default: new Footer({ children: [new Paragraph({ 
        alignment: AlignmentType.CENTER,
        children: [new TextRun({ text: "\u2014 ", color: colors.secondary }), new TextRun({ children: [PageNumber.CURRENT], color: colors.secondary }), new TextRun({ text: " \u2014", color: colors.secondary })]
      })] })
    },
    children: [
      // Title
      new Paragraph({ heading: HeadingLevel.TITLE, children: [new TextRun("CIS \u9879\u76EE\u6DF1\u5EA6\u4EE3\u7801\u5BA1\u67E5\u62A5\u544A")] }),
      new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 400 },
        children: [new TextRun({ text: "\u5BA1\u67E5\u65E5\u671F: 2026-02-16 | \u9879\u76EE\u7248\u672C: v1.1.5", color: colors.secondary, size: 22 })] }),
      
      // Executive Summary
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E00\u3001\u6267\u884C\u6458\u8981")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("CIS (Cluster of Independent Systems) \u662F\u4E00\u4E2A\u57FA\u4E8E Rust \u7684\u5206\u5E03\u5F0F\u8BA1\u7B97\u7CFB\u7EDF\uFF0C\u91C7\u7528\u786C\u4EF6\u7ED1\u5B9A\u578B\u8EAB\u4EFD\u8BA4\u8BC1\u548C P2P \u8054\u90A6\u67B6\u6784\u3002\u672C\u6B21\u5BA1\u67E5\u6DB5\u76D6\u4EE3\u7801\u67B6\u6784\u3001\u5B89\u5168\u6027\u3001\u6027\u80FD\u3001\u6D4B\u8BD5\u8986\u76D6\u7387\u7B49\u591A\u4E2A\u7EF4\u5EA6\uFF0C\u5BF9\u9879\u76EE\u8FDB\u884C\u4E86\u5168\u9762\u6DF1\u5EA6\u5206\u6790\u3002\u9879\u76EE\u6574\u4F53\u4EE3\u7801\u91CF\u7EA6 25,000+ \u884C Rust \u4EE3\u7801\uFF0C\u5305\u542B 19 \u4E2A workspace \u6210\u5458\uFF0C\u67B6\u6784\u8BBE\u8BA1\u6E05\u6670\uFF0C\u6A21\u5757\u5316\u7A0B\u5EA6\u9AD8\u3002")] }),
      
      // Key Findings Table
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("1.1 \u5173\u952E\u53D1\u73B0\u6982\u89C8")] }),
      new Table({
        columnWidths: [2340, 2340, 4680],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5BA1\u67E5\u7EF4\u5EA6", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8BC4\u4EF7", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4E3B\u8981\u53D1\u73B0", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u67B6\u6784\u8BBE\u8BA1")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4F18\u79C0 \u2605\u2605\u2605\u2605\u2605", color: "16A34A" })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6A21\u5757\u5316\u7A0B\u5EA6\u9AD8\uFF0C\u804C\u8D23\u5206\u79BB\u6E05\u6670\uFF0C\u7B26\u5408 SOLID \u539F\u5219")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u5B89\u5168\u6027")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u826F\u597D \u2605\u2605\u2605\u2605\u25CB", color: "CA8A04" })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("WASM \u6C99\u7BB1\u5B9E\u73B0\u5B8C\u5584\uFF0C\u5A01\u80C1\u6A21\u578B\u8BE6\u7EC6\uFF0C\u547D\u4EE4\u6CE8\u5165\u9632\u62A4\u5F85\u52A0\u5F3A")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u4EE3\u7801\u8D28\u91CF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u826F\u597D \u2605\u2605\u2605\u2605\u25CB", color: "CA8A04" })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6587\u6863\u5B8C\u5584\uFF0C\u9519\u8BEF\u5904\u7406\u89C4\u8303\uFF0C\u90E8\u5206 unsafe \u4EE3\u7801\u9700\u5BA1\u67E5")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u6D4B\u8BD5\u8986\u76D6")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4E2D\u7B49 \u2605\u2605\u2605\u25CB\u25CB", color: "EA580C" })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("57 \u4E2A\u6D4B\u8BD5\u6587\u4EF6\uFF0C\u5355\u5143\u6D4B\u8BD5\u8F83\u591A\uFF0C\u96C6\u6210\u6D4B\u8BD5\u6709\u5F85\u589E\u52A0")] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 1: \u5BA1\u67E5\u7ED3\u679C\u6982\u89C8", size: 20, color: colors.secondary })] }),
      
      // Architecture Review
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E8C\u3001\u67B6\u6784\u8BBE\u8BA1\u5BA1\u67E5")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("2.1 \u9879\u76EE\u7ED3\u6784")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u91C7\u7528 Cargo Workspace \u7EC4\u7EC7\u591A\u4E2A crate\uFF0C\u5171\u5305\u542B 19 \u4E2A\u6210\u5458\u3002\u6838\u5FC3\u67B6\u6784\u5206\u4E3A\u4E09\u5C42\uFF1A\u8868\u73B0\u5C42\u3001\u63A7\u5236\u5C42\u548C\u7269\u7406\u5C42\u3002\u8FD9\u79CD\u5206\u5C42\u8BBE\u8BA1\u4F7F\u5F97\u5404\u5C42\u804C\u8D23\u660E\u786E\uFF0C\u4FBF\u4E8E\u7EF4\u62A4\u548C\u6D4B\u8BD5\u3002\u8868\u73B0\u5C42\u5305\u542B GUI \u548C CLI \u63A5\u53E3\uFF0C\u63A7\u5236\u5C42\u5305\u542B DAG \u8C03\u5EA6\u5668\u548C\u6267\u884C\u5668\uFF0C\u7269\u7406\u5C42\u5305\u542B\u5B58\u50A8\u548C P2P \u7F51\u7EDC\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("2.2 \u6838\u5FC3\u6A21\u5757\u5206\u6790")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("cis-core \u6838\u5FC3\u5E93")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("cis-core \u662F\u9879\u76EE\u7684\u6838\u5FC3\u5E93\uFF0C\u63D0\u4F9B\u6240\u6709\u57FA\u7840\u8BBE\u65BD\u3002\u5305\u542B 30+ \u4E2A\u5B50\u6A21\u5757\uFF0C\u6DB5\u76D6\u7C7B\u578B\u5B9A\u4E49\u3001\u9519\u8BEF\u5904\u7406\u3001\u8C03\u5EA6\u5668\u3001\u5B58\u50A8\u3001 P2P \u7F51\u7EDC\u3001 WASM \u8FD0\u884C\u65F6\u3001 Agent \u96C6\u6210\u7B49\u3002\u6A21\u5757\u8BBE\u8BA1\u9075\u5FAA\u201C\u6700\u5C0F\u4F9D\u8D56\u201D\u539F\u5219\uFF0C\u901A\u8FC7 feature flags \u63A7\u5236\u529F\u80FD\u5F00\u542F\uFF0C\u652F\u6301\u6309\u9700\u7F16\u8BD1\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u8C03\u5EA6\u5668\u6A21\u5757 (scheduler)")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u8C03\u5EA6\u5668\u6A21\u5757\u5B9E\u73B0\u4E86\u5B8C\u6574\u7684 DAG \u4EFB\u52A1\u8C03\u5EA6\u7CFB\u7EDF\u3002\u652F\u6301\u4EFB\u52A1\u4F9D\u8D56\u7BA1\u7406\u3001\u5468\u671F\u68C0\u6D4B\u3001\u62D3\u6251\u6392\u5E8F\u3001\u5931\u8D25\u4F20\u64AD\u3001\u5E76\u884C\u6267\u884C\u7B49\u6838\u5FC3\u529F\u80FD\u3002\u91C7\u7528\u201C\u56DB\u5C42\u51B3\u7B56\u201D\u673A\u5236\uFF08Mechanical/Recommended/Confirmed/Arbitrated\uFF09\uFF0C\u6839\u636E\u4EFB\u52A1\u7EA7\u522B\u81EA\u52A8\u9009\u62E9\u6267\u884C\u7B56\u7555\u3002\u8BBE\u8BA1\u501F\u9274\u4E86 AgentFlow \u7684\u6210\u719F\u5B9E\u73B0\uFF0C\u7A33\u5B9A\u6027\u8F83\u9AD8\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("P2P \u7F51\u7EDC\u6A21\u5757 (p2p)")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("P2P \u6A21\u5757\u5B9E\u73B0\u4E86\u5B8C\u6574\u7684\u5206\u5E03\u5F0F\u8282\u70B9\u901A\u4FE1\u80FD\u529B\u3002\u5305\u542B QUIC \u4F20\u8F93\u5C42\u3001 Kademlia DHT \u8DEF\u7531\u3001 mDNS \u53D1\u73B0\u3001 NAT \u7A7F\u900F\u3001 Noise Protocol \u52A0\u5BC6\u7B49\u5173\u952E\u7EC4\u4EF6\u3002\u91C7\u7528 CRDT \u6570\u636E\u7ED3\u6784\u5B9E\u73B0\u5206\u5E03\u5F0F\u72B6\u6001\u540C\u6B65\uFF0C\u652F\u6301 LWW-Register\u3001 G-Counter\u3001 PN-Counter\u3001 OR-Set \u7B49\u7C7B\u578B\u3002")] }),
      
      // Security Review
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E09\u3001\u5B89\u5168\u6027\u5BA1\u67E5")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.1 \u5A01\u80C1\u6A21\u578B\u5206\u6790")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u5DF2\u5EFA\u7ACB\u8BE6\u7EC6\u7684\u5A01\u80C1\u6A21\u578B\u6587\u6863\uFF0C\u8BC6\u522B\u4E86 15 \u4E2A\u4E3B\u8981\u5A01\u80C1\uFF0C\u5305\u542B 5 \u4E2A P0 \u7EA7\u4E25\u91CD\u5A01\u80C1\u548C 6 \u4E2A P1 \u7EA7\u9AD8\u5371\u5A01\u80C1\u3002\u5A01\u80C1\u6A21\u578B\u6DB5\u76D6\u7CFB\u7EDF\u8FB9\u754C\u3001\u653B\u51FB\u9762\u679A\u4E3E\u3001\u653B\u51FB\u5411\u91CF\u5206\u6790\u548C\u98CE\u9669\u8BC4\u4F30\u77E9\u9635\uFF0C\u4F53\u73B0\u4E86\u5B89\u5168\u56E2\u961F\u7684\u4E13\u4E1A\u6027\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.2 WASM \u6C99\u7BB1\u5B89\u5168")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("WASM \u6C99\u7BB1\u5B9E\u73B0\u4E86\u591A\u5C42\u5B89\u5168\u9632\u62A4\uFF1A\u8DEF\u5F84\u767D\u540D\u5355\u9A8C\u8BC1\u3001\u8DEF\u5F84\u904D\u5386\u653B\u51FB\u9632\u62A4\u3001\u7B26\u53F7\u94FE\u63A5\u9003\u9038\u68C0\u6D4B\u3001\u6587\u4EF6\u63CF\u8FF0\u7B26\u9650\u5236\u3001\u78C1\u76D8\u914D\u989D\u9650\u5236\u3002\u91C7\u7528 RAII \u6A21\u5F0F\u7BA1\u7406\u6587\u4EF6\u63CF\u8FF0\u7B26\uFF0C\u786E\u4FDD\u8D44\u6E90\u81EA\u52A8\u91CA\u653E\uFF0C\u9632\u6B62\u8D44\u6E90\u6CC4\u6F0F\u3002\u8FD9\u662F P0 \u7EA7\u5B89\u5168\u4FEE\u590D\u7684\u91CD\u8981\u6210\u679C\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.3 \u52A0\u5BC6\u4E0E\u8EAB\u4EFD\u8BA4\u8BC1")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u91C7\u7528 Ed25519 \u6570\u5B57\u7B7E\u540D\u8FDB\u884C\u8EAB\u4EFD\u8BA4\u8BC1\uFF0C X25519 \u8FDB\u884C\u5BC6\u94AE\u4EA4\u6362\uFF0C ChaCha20-Poly1305 \u8FDB\u884C\u5BF9\u79F0\u52A0\u5BC6\u3002 DID \u7BA1\u7406\u6A21\u5757\u5B9E\u73B0\u4E86\u57FA\u4E8E\u786C\u4EF6\u6307\u7EB9\u7684\u8EAB\u4EFD\u7ED1\u5B9A\uFF0C\u79C1\u94A5\u6587\u4EF6\u6743\u9650\u8BBE\u7F6E\u4E3A 0600\uFF0C\u9632\u6B62\u672A\u6388\u6743\u8BBF\u95EE\u3002\u52A0\u5BC6\u6A21\u5757\u652F\u6301 V2 \u7248\u672C\u7684\u5BC6\u94A5\u683C\u5F0F\uFF0C\u63D0\u4F9B\u66F4\u597D\u7684\u5411\u540E\u517C\u5BB9\u6027\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.4 \u5B89\u5168\u5EFA\u8BAE")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "\u547D\u4EE4\u6CE8\u5165\u9632\u62A4\uFF1A", bold: true }), new TextRun("Agent \u6267\u884C\u5668\u7684\u547D\u4EE4\u9A8C\u8BC1\u6846\u67B6\u5DF2\u8BBE\u8BA1\uFF0C\u4F46\u5B9E\u73B0\u5F85\u5B8C\u5584\u3002\u5EFA\u8BAE\u589E\u52A0\u547D\u4EE4\u767D\u540D\u5355\u7684\u5355\u5143\u6D4B\u8BD5\u8986\u76D6\uFF0C\u786E\u4FDD\u6240\u6709\u5371\u9669\u547D\u4EE4\u90FD\u88AB\u6B63\u786E\u62E6\u622A\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "unsafe \u4EE3\u7801\u5BA1\u67E5\uFF1A", bold: true }), new TextRun("\u9879\u76EE\u4E2D\u5B58\u5728\u7EA6 21 \u5904 unsafe \u4EE3\u7801\u5757\uFF0C\u4E3B\u8981\u96C6\u4E2D\u5728 PID \u7BA1\u7406\u3001\u5185\u5B58\u670D\u52A1\u3001\u5411\u91CF\u5B58\u50A8\u7B49\u5E95\u5C42\u6A21\u5757\u3002\u5EFA\u8BAE\u5BF9\u6BCF\u5904 unsafe \u4EE3\u7801\u8FDB\u884C\u5B89\u5168\u5BA1\u67E5\uFF0C\u6DFB\u52A0 SAFETY \u6CE8\u91CA\u8BF4\u660E\u4E0D\u53D8\u91CF\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u5907\u4EFD\u52A0\u5BC6\uFF1A", bold: true }), new TextRun("\u5A01\u80C1\u6A21\u578B\u4E2D\u6807\u8BC6\u5907\u4EFD\u6587\u4EF6\u672A\u52A0\u5BC6\u4E3A P0 \u7EA7\u5A01\u80C1\u3002\u5EFA\u8BAE\u5B9E\u73B0\u81EA\u52A8\u5907\u4EFD\u52A0\u5BC6\u529F\u80FD\uFF0C\u786E\u4FDD\u5907\u4EFD\u6570\u636E\u7684\u5B89\u5168\u6027\u3002")] }),
      
      // Code Quality
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u56DB\u3001\u4EE3\u7801\u8D28\u91CF\u5BA1\u67E5")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("4.1 \u9519\u8BEF\u5904\u7406")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u91C7\u7528\u7EDF\u4E00\u7684\u9519\u8BEF\u5904\u7406\u67B6\u6784\uFF0C\u5305\u542B unified \u548C legacy \u4E24\u5957\u7CFB\u7EDF\u3002\u65B0\u7684 unified \u7CFB\u7EDF\u63D0\u4F9B\u4E30\u5BCC\u7684\u9519\u8BEF\u4E0A\u4E0B\u6587\u3001\u5206\u7C7B\u548C\u4E25\u91CD\u7EA7\u522B\u3002\u652F\u6301\u9519\u8BEF\u94FE\u3001\u9519\u8BEF\u8F6C\u6362\u548C\u6062\u590D\u6027\u5224\u65AD\u3002\u8FD9\u79CD\u8BBE\u8BA1\u4F7F\u5F97\u9519\u8BEF\u8BCA\u65AD\u548C\u5904\u7406\u66F4\u52A0\u65B9\u4FBF\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("4.2 \u6587\u6863\u8D28\u91CF")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4EE3\u7801\u6587\u6863\u8D28\u91CF\u8F83\u9AD8\uFF0C\u5927\u90E8\u5206\u6A21\u5757\u90FD\u6709\u8BE6\u7EC6\u7684\u6A21\u5757\u7EA7\u6587\u6863\uFF0C\u8BF4\u660E\u529F\u80FD\u3001\u7279\u6027\u548C\u4F7F\u7528\u793A\u4F8B\u3002\u5173\u952E\u51FD\u6570\u90FD\u6709\u6587\u6863\u6CE8\u91CA\uFF0C\u5305\u542B\u53C2\u6570\u8BF4\u660E\u3001\u8FD4\u56DE\u503C\u548C\u793A\u4F8B\u4EE3\u7801\u3002\u9879\u76EE\u8FD8\u5305\u542B\u5927\u91CF\u7684\u8BBE\u8BA1\u6587\u6863\uFF0C\u5305\u542B\u67B6\u6784\u8BBE\u8BA1\u3001\u5A01\u80C1\u6A21\u578B\u3001\u90E8\u7F72\u6307\u5357\u7B49\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("4.3 \u4EE3\u7801\u7EC4\u7EC7")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4EE3\u7801\u7EC4\u7EC7\u6E05\u6670\uFF0C\u6BCF\u4E2A\u6A21\u5757\u804C\u8D23\u5355\u4E00\u3002\u91C7\u7528 Builder \u6A21\u5F0F\u6784\u5EFA\u590D\u6742\u5BF9\u8C61\uFF0C\u5982 WasiSandbox\u3001AgentTaskBuilder \u7B49\u3002\u4F7F\u7528 trait \u5B9A\u4E49\u62BD\u8C61\u63A5\u53E3\uFF0C\u5982 AgentProvider\u3001MemoryServiceTrait \u7B49\uFF0C\u4FBF\u4E8E\u6269\u5C55\u548C\u6D4B\u8BD5\u3002\u4F9D\u8D56\u6CE8\u5165\u901A\u8FC7 container \u6A21\u5757\u5B9E\u73B0\uFF0C\u652F\u6301\u6D4B\u8BD5\u65F6\u7684 Mock \u5BF9\u8C61\u6CE8\u5165\u3002")] }),
      
      // Test Coverage
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E94\u3001\u6D4B\u8BD5\u8986\u76D6\u7387\u5BA1\u67E5")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("5.1 \u6D4B\u8BD5\u6587\u4EF6\u5206\u5E03")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u5305\u542B 23 \u4E2A tests \u76EE\u5F55\u4E0B\u7684\u6D4B\u8BD5\u6587\u4EF6\u300133 \u4E2A *_test*.rs \u6587\u4EF6\u548C 1 \u4E2A test_*.rs \u6587\u4EF6\uFF0C\u603B\u8BA1\u7EA6 57 \u4E2A\u6D4B\u8BD5\u6587\u4EF6\u3002\u6D4B\u8BD5\u7C7B\u578B\u5305\u542B\u5355\u5143\u6D4B\u8BD5\u3001\u96C6\u6210\u6D4B\u8BD5\u3001\u5B89\u5168\u6D4B\u8BD5\u3001\u6027\u80FD\u57FA\u51C6\u6D4B\u8BD5\u7B49\u3002\u9879\u76EE\u8FD8\u5305\u542B Fuzz \u6D4B\u8BD5\u6846\u67B6\uFF0C\u7528\u4E8E\u53D1\u73B0\u6F5C\u5728\u7684\u5B89\u5168\u6F0F\u6D1E\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("5.2 \u6D4B\u8BD5\u8D28\u91CF\u5206\u6790")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun({ text: "\u4F18\u70B9\uFF1A", bold: true }), new TextRun("\u5355\u5143\u6D4B\u8BD5\u8F83\u4E3A\u5B8C\u5584\uFF0C\u5C24\u5176\u662F WASM \u6C99\u7BB1\u3001\u8C03\u5EA6\u5668\u7B49\u6838\u5FC3\u6A21\u5757\u3002\u6D4B\u8BD5\u7528\u4F8B\u8BBE\u8BA1\u5408\u7406\uFF0C\u8986\u76D6\u4E86\u6B63\u5E38\u8DEF\u5F84\u548C\u5F02\u5E38\u8DEF\u5F84\u3002\u4F7F\u7528 Mock \u5BF9\u8C61\u8FDB\u884C\u9694\u79BB\u6D4B\u8BD5\uFF0C\u63D0\u9AD8\u4E86\u6D4B\u8BD5\u6548\u7387\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u4E0D\u8DB3\uFF1A", bold: true }), new TextRun("\u96C6\u6210\u6D4B\u8BD5\u76F8\u5BF9\u8F83\u5C11\uFF0C\u5C24\u5176\u662F P2P \u7F51\u7EDC\u3001\u7AEF\u5230\u7AEF\u901A\u4FE1\u7B49\u590D\u6742\u573A\u666F\u3002\u7F3A\u5C11\u6301\u7EED\u96C6\u6210\u6D4B\u8BD5\u6D41\u6C34\u7EBF\u3002\u5EFA\u8BAE\u589E\u52A0\u7AEF\u5230\u7AEF\u6D4B\u8BD5\u7528\u4F8B\uFF0C\u8986\u76D6\u5178\u578B\u4E1A\u52A1\u573A\u666F\u3002")] }),
      
      // Performance
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u516D\u3001\u6027\u80FD\u5206\u6790")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("6.1 \u57FA\u51C6\u6D4B\u8BD5")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u5305\u542B\u591A\u4E2A\u6027\u80FD\u57FA\u51C6\u6D4B\u8BD5\uFF0C\u4F4D\u4E8E benches/ \u76EE\u5F55\u4E0B\u3002\u6DB5\u76D6 agent_teams_benchmarks\u3001database_operations\u3001dag_operations\u3001weekly_archived_memory\u3001task_manager \u7B49\u591A\u4E2A\u65B9\u9762\u3002\u4F7F\u7528 Criterion \u5E93\u8FDB\u884C\u4E13\u4E1A\u7684\u6027\u80FD\u6D4B\u8BD5\uFF0C\u652F\u6301\u7EDF\u8BA1\u5206\u6790\u548C\u56DE\u5F52\u5206\u6790\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("6.2 \u5E76\u53D1\u8BBE\u8BA1")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u9879\u76EE\u91C7\u7528 Tokio \u5F02\u6B65\u8FD0\u884C\u65F6\uFF0C\u652F\u6301\u9AD8\u5E76\u53D1\u573A\u666F\u3002\u4F7F\u7528 DashMap \u5B9E\u73B0\u9AD8\u6027\u80FD\u5E76\u53D1\u54C8\u5E0C\u8868\u3002\u8FDE\u63A5\u6C60\u3001\u5185\u5B58\u6C60\u7B49\u8D44\u6E90\u7BA1\u7406\u6A21\u5757\u90FD\u8003\u8651\u4E86\u5E76\u53D1\u5B89\u5168\u3002\u9501\u8D85\u65F6\u6A21\u5757\u63D0\u4F9B\u4E86\u6B7B\u9501\u68C0\u6D4B\u548C\u8D85\u65F6\u673A\u5236\uFF0C\u9632\u6B62\u8D44\u6E90\u6B7B\u9501\u3002")] }),
      
      // Improvement Suggestions
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E03\u3001\u6539\u8FDB\u5EFA\u8BAE")] }),
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("7.1 \u9AD8\u4F18\u5148\u7EA7\u5EFA\u8BAE")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "\u5B8C\u5584\u547D\u4EE4\u6CE8\u5165\u9632\u62A4\uFF1A", bold: true }), new TextRun("\u5F53\u524D\u547D\u4EE4\u9A8C\u8BC1\u6846\u67B6\u5DF2\u8BBE\u8BA1\uFF0C\u4F46\u5B9E\u73B0\u4E0D\u5B8C\u5584\u3002\u5EFA\u8BAE\u5B8C\u6210\u547D\u4EE4\u767D\u540D\u5355\u7684\u5B8C\u6574\u5B9E\u73B0\uFF0C\u589E\u52A0\u5355\u5143\u6D4B\u8BD5\u8986\u76D6\uFF0C\u786E\u4FDD\u6240\u6709\u5371\u9669\u547D\u4EE4\u90FD\u88AB\u6B63\u786E\u62E6\u622A\u3002\u8FD9\u662F P0 \u7EA7\u5B89\u5168\u5A01\u80C1\u7684\u5173\u952E\u7F13\u89E3\u63AA\u65BD\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "\u589E\u52A0\u96C6\u6210\u6D4B\u8BD5\u8986\u76D6\uFF1A", bold: true }), new TextRun("\u5F53\u524D\u96C6\u6210\u6D4B\u8BD5\u76F8\u5BF9\u8F83\u5C11\u3002\u5EFA\u8BAE\u589E\u52A0 P2P \u7F51\u7EDC\u901A\u4FE1\u3001\u591A\u8282\u70B9\u534F\u4F5C\u3001\u7AEF\u5230\u7AEF\u4E1A\u52A1\u6D41\u7A0B\u7B49\u573A\u666F\u7684\u96C6\u6210\u6D4B\u8BD5\u3002\u8003\u8651\u5F15\u5165\u6301\u7EED\u96C6\u6210\u6D4B\u8BD5\u6D41\u6C34\u7EBF\uFF0C\u786E\u4FDD\u4EE3\u7801\u5408\u5E76\u540E\u7684\u8D28\u91CF\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u5B9E\u73B0\u5907\u4EFD\u52A0\u5BC6\u529F\u80FD\uFF1A", bold: true }), new TextRun("\u5A01\u80C1\u6A21\u578B\u4E2D\u6807\u8BC6\u5907\u4EFD\u6587\u4EF6\u672A\u52A0\u5BC6\u4E3A P0 \u7EA7\u5A01\u80C1\u3002\u5EFA\u8BAE\u5B9E\u73B0\u81EA\u52A8\u5907\u4EFD\u52A0\u5BC6\u529F\u80FD\uFF0C\u4F7F\u7528\u7528\u6237\u5BC6\u7801\u6D3E\u751F\u52A0\u5BC6\u5BC6\u94A5\uFF0C\u786E\u4FDD\u5907\u4EFD\u6570\u636E\u7684\u5B89\u5168\u6027\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("7.2 \u4E2D\u4F18\u5148\u7EA7\u5EFA\u8BAE")] }),
      new Paragraph({ numbering: { reference: "numbered-4", level: 0 }, children: [new TextRun({ text: "unsafe \u4EE3\u7801\u5BA1\u67E5\uFF1A", bold: true }), new TextRun("\u5BF9\u6240\u6709 unsafe \u4EE3\u7801\u5757\u8FDB\u884C\u5B89\u5168\u5BA1\u67E5\uFF0C\u6DFB\u52A0 SAFETY \u6CE8\u91CA\u8BF4\u660E\u4E0D\u53D8\u91CF\u548C\u5B89\u5168\u4FDD\u8BC1\u3002\u8003\u8651\u4F7F\u7528 MinSafe \u7B49\u5DE5\u5177\u8FDB\u884C\u81EA\u52A8\u5316\u5BA1\u67E5\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-4", level: 0 }, children: [new TextRun({ text: "\u4F9D\u8D56\u66F4\u65B0\uFF1A", bold: true }), new TextRun("\u90E8\u5206\u4F9D\u8D56\u7248\u672C\u8F83\u65E7\uFF0C\u5982 tokio 1.35\u3001rusqlite 0.32 \u7B49\u3002\u5EFA\u8BAE\u5B9A\u671F\u8FDB\u884C\u4F9D\u8D56\u66F4\u65B0\uFF0C\u4FEE\u590D\u5DF2\u77E5\u6F0F\u6D1E\u3002\u4F7F\u7528 cargo audit \u8FDB\u884C\u5B9A\u671F\u5B89\u5168\u626B\u63CF\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-4", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u65E5\u5FD7\u89C4\u8303\u5316\uFF1A", bold: true }), new TextRun("\u5F53\u524D\u65E5\u5FD7\u7CFB\u7EDF\u4F7F\u7528 tracing \u5E93\uFF0C\u4F46\u65E5\u5FD7\u7EA7\u522B\u548C\u683C\u5F0F\u4E0D\u591F\u7EDF\u4E00\u3002\u5EFA\u8BAE\u5236\u5B9A\u65E5\u5FD7\u89C4\u8303\uFF0C\u660E\u786E\u5404\u7EA7\u522B\u7684\u4F7F\u7528\u573A\u666F\uFF0C\u65B9\u4FBF\u95EE\u9898\u8BCA\u65AD\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("7.3 \u4F4E\u4F18\u5148\u7EA7\u5EFA\u8BAE")] }),
      new Paragraph({ numbering: { reference: "numbered-5", level: 0 }, children: [new TextRun({ text: "\u6587\u6863\u6574\u7406\uFF1A", bold: true }), new TextRun("\u9879\u76EE\u5305\u542B\u5927\u91CF\u6587\u6863\uFF0C\u4F46\u90E8\u5206\u6587\u6863\u5DF2\u8FC7\u65F6\u3002\u5EFA\u8BAE\u6E05\u7406\u8FC7\u65F6\u6587\u6863\uFF0C\u66F4\u65B0\u67B6\u6784\u6587\u6863\uFF0C\u4FDD\u6301\u6587\u6863\u4E0E\u4EE3\u7801\u540C\u6B65\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-5", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u4EE3\u7801\u6CE8\u91CA\u8865\u5145\uFF1A", bold: true }), new TextRun("\u90E8\u5206\u590D\u6742\u4E1A\u52A1\u903B\u8F91\u7F3A\u5C11\u8BE6\u7EC6\u6CE8\u91CA\u3002\u5EFA\u8BAE\u8865\u5145\u4E1A\u52A1\u903B\u8F91\u8BF4\u660E\uFF0C\u65B9\u4FBF\u540E\u7EED\u7EF4\u62A4\u548C\u65B0\u6210\u5458\u4E0A\u624B\u3002")] }),
      
      // Conclusion
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u516B\u3001\u603B\u7ED3")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("CIS \u9879\u76EE\u6574\u4F53\u8D28\u91CF\u8F83\u9AD8\uFF0C\u67B6\u6784\u8BBE\u8BA1\u5408\u7406\uFF0C\u6A21\u5757\u5316\u7A0B\u5EA6\u9AD8\u3002\u5B89\u5168\u65B9\u9762\uFF0C\u5A01\u80C1\u6A21\u578B\u5B8C\u5584\uFF0C WASM \u6C99\u7BB1\u5B9E\u73B0\u5230\u4F4D\uFF0C\u52A0\u5BC6\u8EAB\u4EFD\u8BA4\u8BC1\u4F53\u7CFB\u5B8C\u6574\u3002\u4E3B\u8981\u9700\u8981\u5173\u6CE8\u7684\u65B9\u9762\u5305\u62EC\uFF1A\u547D\u4EE4\u6CE8\u5165\u9632\u62A4\u7684\u5B8C\u5584\u3001\u96C6\u6210\u6D4B\u8BD5\u8986\u76D6\u7387\u7684\u63D0\u5347\u3001\u5907\u4EFD\u52A0\u5BC6\u529F\u80FD\u7684\u5B9E\u73B0\u3002\u5EFA\u8BAE\u6309\u7167\u4F18\u5148\u7EA7\u987A\u5E8F\u9010\u6B65\u843D\u5B9E\u6539\u8FDB\u63AA\u65BD\uFF0C\u6301\u7EED\u63D0\u5347\u9879\u76EE\u8D28\u91CF\u3002")] }),
      
      // Appendix
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u9644\u5F55\uFF1A\u5BA1\u67E5\u7EDF\u8BA1")] }),
      new Table({
        columnWidths: [4680, 4680],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u6307\u6807", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u6570\u503C", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Workspace \u6210\u5458\u6570\u91CF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("19 \u4E2A")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u4EE3\u7801\u884C\u6570\uFF08\u4E3B\u8981\u6E90\u7801\uFF09")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("~25,000 \u884C")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6D4B\u8BD5\u6587\u4EF6\u6570\u91CF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("57 \u4E2A")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("unsafe \u4EE3\u7801\u5757\u6570\u91CF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("~21 \u5904")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6838\u5FC3\u6A21\u5757\u6570\u91CF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("30+ \u4E2A")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5A01\u80C1\u6A21\u578B\u8BC6\u522B\u5A01\u80C1\u6570")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("15 \u4E2A")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6027\u80FD\u57FA\u51C6\u6D4B\u8BD5\u6570\u91CF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 4680, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.RIGHT, children: [new TextRun("7 \u4E2A")] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 2: \u5BA1\u67E5\u7EDF\u8BA1\u6570\u636E", size: 20, color: colors.secondary })] })
    ]
  }]
});

Packer.toBuffer(doc).then(buffer => {
  fs.writeFileSync("/home/z/my-project/download/CIS_Code_Review_Report.docx", buffer);
  console.log("Report generated: /home/z/my-project/download/CIS_Code_Review_Report.docx");
});
