const { Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell, 
        Header, Footer, AlignmentType, LevelFormat, HeadingLevel, BorderStyle, 
        WidthType, ShadingType, VerticalAlign, PageNumber, PageBreak } = require('docx');
const fs = require('fs');

const colors = {
  primary: "1A1F16",
  body: "2D3329",
  secondary: "4A5548",
  accent: "94A3B8",
  tableBg: "F8FAF7",
  success: "16A34A",
  warning: "CA8A04",
  danger: "DC2626",
  info: "2563EB"
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
        children: [new TextRun({ text: "CIS+OpenClaw \u4F01\u5212\u8BC4\u5224\u4E0E\u8865\u5145", font: "SimHei", size: 20, color: colors.secondary })]
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
      new Paragraph({ heading: HeadingLevel.TITLE, children: [new TextRun("CIS \u6574\u5408 OpenClaw \u4F01\u5212\u8BC4\u5224\u4E0E\u8865\u5145\u5EFA\u8BAE")] }),
      new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 400 },
        children: [new TextRun({ text: "\u8BC4\u5224\u65E5\u671F: 2026-02-16 | \u8BC4\u5224\u4EBA: \u6DF1\u5EA6\u4EE3\u7801\u5BA1\u67E5\u7CFB\u7EDF", color: colors.secondary, size: 22 })] }),
      
      // Executive Summary
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E00\u3001\u603B\u4F53\u8BC4\u5224")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("1.1 \u4F01\u5212\u8D28\u91CF\u8BC4\u4F30")] }),
      new Table({
        columnWidths: [2340, 2340, 2340, 2340],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u6587\u6863", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5B8C\u6574\u5EA6", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u6DF1\u5EA6", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8BC4\u4EF7", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u7EFC\u5408\u5BA1\u67E5\u62A5\u544A")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u2605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4F18\u79C0", color: colors.success })] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("OpenClaw\u96C6\u6210\u5206\u6790")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u2605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4F18\u79C0", color: colors.success })] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Skill\u517C\u5BB9\u65B9\u6848")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u2605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u2605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4F18\u79C0", color: colors.success })] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u529F\u80FD\u8986\u76D6\u5206\u6790")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u826F\u597D", color: colors.warning })] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5B89\u5168\u5BA1\u8BA1\u62A5\u544A")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u25CB\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u826F\u597D", color: colors.warning })] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 1: \u4F01\u5212\u6587\u6863\u8D28\u91CF\u8BC4\u4F30", size: 20, color: colors.secondary })] }),
      
      // Key Findings
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("1.2 \u6838\u5FC3\u8BC4\u5224\u7ED3\u8BBA")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u65B9\u6845\u9009\u62E9\uFF1A", bold: true }), new TextRun({ text: "\u540C\u610F\u201C\u76F4\u63A5\u63A5\u5165OpenClaw Skill\u201D\u65B9\u6848\u4F18\u4E8E\u201CGateway\u96C6\u6210\u201D\u65B9\u6848\u3002", color: colors.success }), new TextRun("\u8FD9\u4E00\u5224\u65AD\u662F\u6B63\u786E\u7684\uFF0C\u7406\u7531\u5145\u5206\uFF1A\u6280\u672F\u6808\u540C\u6784\uFF08WASM\u8FD0\u884C\u65F6\uFF09\u3001\u5F00\u53D1\u5468\u671F\u77ED\uFF084-6\u5468 vs 11\u5468\uFF09\u3001\u7EF4\u62A4\u6210\u672C\u4F4E\uFF08Rust\u5355\u6808\uFF09\u3002")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u5DE5\u4F5C\u91CF\u4F30\u7B97\uFF1A", bold: true }), new TextRun({ text: "4-6\u5468\u7684\u4F30\u7B97\u504F\u4E50\u89C2\u3002", color: colors.warning }), new TextRun("\u5B9E\u9645\u5F00\u53D1\u4E2D\u9700\u8981\u8003\u8651\uFF1A\uFF081\uFF09Skill\u89E3\u6790\u5668\u7684\u8FB9\u7F18\u60C5\u51B5\u5904\u7406\uFF1B\uFF082\uFF09\u5DE5\u5177\u6620\u5C04\u7684\u5B8C\u6574\u6027\uFF1B\uFF083\uFF09\u4E0ECIS\u73B0\u6709\u6A21\u5757\u7684\u6DF1\u5EA6\u96C6\u6210\u3002\u5EFA\u8BAE\u8C03\u6574\u4E3A6-8\u5468\u3002")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u529F\u80FD\u8986\u76D6\u5EA6\uFF1A", bold: true }), new TextRun({ text: "85-90%\u7684\u4F30\u7B97\u57FA\u672C\u5408\u7406\u3002", color: colors.info }), new TextRun("\u4F46\u9700\u8981\u6CE8\u610F\uFF0C\u201C\u6D88\u606F\u6E20\u9053\u5C42\u201D\u4E0D\u4EC5\u4EC5\u662F\u63A5\u5165API\uFF0C\u8FD8\u5305\u542B\u6D88\u606F\u683C\u5F0F\u8F6C\u6362\u3001\u5A92\u4F53\u5904\u7406\u3001\u4E8B\u4EF6\u540C\u6B65\u7B49\u590D\u6742\u903B\u8F91\u3002")] }),
      
      // Critical Issues
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E8C\u3001\u5173\u952E\u95EE\u9898\u8865\u5145")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("2.1 \u4F01\u5212\u4E2D\u672A\u6D89\u53CA\u7684\u5173\u952E\u95EE\u9898")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u95EE\u98981: \u5F31\u7F51\u573A\u666F\u9002\u914D\u6027\u7F3A\u5931")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4F01\u5212\u5B8C\u5168\u672A\u6D89\u53CA\u201C\u6570\u5B57\u6E38\u6C11\u5F31\u7F51\u5F02\u6784\u7F16\u8BD1\u201D\u573A\u666F\u3002\u8FD9\u662F\u4E00\u4E2A\u91CD\u8981\u7684\u4F7F\u7528\u573A\u666F\uFF0C\u5BF9\u4E8E\u6E38\u620F\u5F00\u53D1\u8005\u3001\u8FDC\u7A0B\u5DE5\u4F5C\u8005\u7B49\u7FA4\u4F53\u5177\u6709\u5F3A\u70C8\u9700\u6C42\u3002")] }),
      new Paragraph({ spacing: { after: 200 },
        children: [new TextRun({ text: "\u5F53\u524DCIS\u7F3A\u5931\u7684\u5173\u952E\u80FD\u529B\uFF1A", bold: true })] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "\u79BB\u7EBF\u6D88\u606F\u961F\u5217", bold: true }), new TextRun(" - \u5F31\u7F51\u73AF\u5883\u4E0B\u6D88\u606F\u65E0\u6CD5\u6301\u4E45\u5316")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "\u5F02\u6784\u4EFB\u52A1\u8DEF\u7531", bold: true }), new TextRun(" - \u65E0\u6CD5\u6307\u5B9A\u201C\u6B64\u4EFB\u52A1\u5FC5\u987B\u5728Mac\u4E0A\u6267\u884C\u201D")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "\u4E91\u7AEF\u4FE1\u4EE4\u670D\u52A1", bold: true }), new TextRun(" - TURN\u670D\u52A1\u5668\u5217\u8868\u4E3A\u7A7A")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "Git\u96C6\u6210", bold: true }), new TextRun(" - \u65E0Webhook\u6216\u4E8B\u4EF6\u76D1\u542C")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u95EE\u98982: Skill\u6267\u884C\u5B89\u5168\u8FB9\u754C")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4F01\u5212\u63D0\u5230\u4E86\u201C\u5F00\u6E90\u8D23\u4EFB\u98CE\u9669\u89C4\u907F\u201D\uFF0C\u4F46\u672A\u6DF1\u5165\u5206\u6790Skill\u6267\u884C\u7684\u5B89\u5168\u8FB9\u754C\u3002OpenClaw Skill\u53EF\u80FD\u5305\u542B\uFF1A")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun("\u5371\u9669\u7684\u7CFB\u7EDF\u547D\u4EE4\uFF08exec\u3001rm -rf\u7B49\uFF09")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun("\u7F51\u7EDC\u8BF7\u6C42\uFF08\u53EF\u80FD\u6CC4\u9732\u654F\u611F\u4FE1\u606F\uFF09")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun("\u6587\u4EF6\u7CFB\u7EDF\u64CD\u4F5C\uFF08\u53EF\u80FD\u8BBF\u95EE\u654F\u611F\u6587\u4EF6\uFF09")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, spacing: { after: 200 }, children: [new TextRun("\u73AF\u5883\u53D8\u91CF\u8BBF\u95EE\uFF08\u53EF\u80FD\u83B7\u53D6API\u5BC6\u94A5\uFF09")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u5EFA\u8BAE\uFF1A", bold: true }), new TextRun("\u9700\u8981\u5B9E\u73B0\u7EC6\u7C92\u5EA6\u7684\u6743\u9650\u63A7\u5236\uFF0C\u7C7B\u4F3C\u4E8EAndroid\u7684\u6743\u9650\u7CFB\u7EDF\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u95EE\u98983: \u6D88\u606F\u6E20\u9053\u5C42\u590D\u6742\u5EA6")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4F01\u5212\u5C06\u201C\u6D88\u606F\u6E20\u9053\u5C42\u201D\u5F00\u53D1\u4F30\u7B97\u4E3A3-4\u5468\uFF0C\u8FD9\u4E2A\u4F30\u7B97\u8FC7\u4E8E\u4E50\u89C2\u3002\u6BCF\u4E2A\u5E73\u53F0\u90FD\u6709\u5176\u7279\u6709\u7684\u590D\u6742\u6027\uFF1A")] }),
      new Table({
        columnWidths: [2340, 3120, 3900],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5E73\u53F0", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u590D\u6742\u70B9", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5EFA\u8BAE\u5DE5\u4F5C\u91CF", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("WhatsApp")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5546\u4E1AAPI\u3001\u6D88\u606F\u6A21\u677F\u3001\u5A92\u4F53\u5904\u7406")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("1-1.5\u5468")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Telegram")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Bot API\u3001Inline\u67E5\u8BE2\u3001WebApp")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("0.5-1\u5468")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Slack")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("App\u6743\u9650\u3001Block Kit\u3001\u4E8B\u4EF6\u8BA2\u9605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("1-1.5\u5468")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Discord")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Gateway\u8FDE\u63A5\u3001Intents\u3001\u4EA4\u4E92\u7EC4\u4EF6")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("1-1.5\u5468")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ children: [new TextRun({ text: "\u603B\u8BA1", bold: true })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ children: [new TextRun("")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ children: [new TextRun({ text: "4-5.5\u5468\uFF08\u4EC5\u6E20\u9053\u5C42\uFF09", bold: true })] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 2: \u6D88\u606F\u6E20\u9053\u5C42\u5F00\u53D1\u5DE5\u4F5C\u91CF\u91CD\u4F30", size: 20, color: colors.secondary })] }),
      
      // Recommendations
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E09\u3001\u8865\u5145\u5EFA\u8BAE")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.1 \u67B6\u6784\u5C42\u8865\u5145")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u5EFA\u8BAE1: \u589E\u52A0\u4EFB\u52A1\u8C03\u5EA6\u4E2D\u95F4\u4EF6")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4E3A\u4E86\u652F\u6301\u201C\u6570\u5B57\u6E38\u6C11\u5F31\u7F51\u5F02\u6784\u7F16\u8BD1\u201D\u573A\u666F\uFF0C\u5EFA\u8BAE\u5728\u73B0\u6709\u67B6\u6784\u4E0A\u589E\u52A0\u4E00\u5C42\u201C\u4EFB\u52A1\u8C03\u5EA6\u4E2D\u95F4\u4EF6\u201D\uFF1A")] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "\u6838\u5FC3\u7EC4\u4EF6\uFF1A", bold: true, font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "- OfflineQueue: \u79BB\u7EBF\u6D88\u606F\u961F\u5217\uFF08SQLite\u6301\u4E45\u5316\uFF09", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "- NodeSelector: \u8282\u70B9\u80FD\u529B\u5339\u914D\uFF08os/arch/gpu\uFF09", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "- TaskRouter: \u4EFB\u52A1\u8DEF\u7531\u5668\uFF08\u5206\u53D1\u5230\u5408\u9002\u8282\u70B9\uFF09", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 200 },
        children: [new TextRun({ text: "- StateTracker: \u4EFB\u52A1\u72B6\u6001\u8DDF\u8E2A\uFF08\u652F\u6301\u65AD\u70B9\u7EED\u4F20\uFF09", font: "SarasaMonoSC", size: 20 })] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u5EFA\u8BAE2: \u589E\u52A0\u4E91\u7AEF\u4FE1\u4EE4\u8282\u70B9")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u63D0\u4F9B\u4E00\u4E2A\u516C\u7F51\u53EF\u8BBF\u95EE\u7684\u4FE1\u4EE4\u8282\u70B9\uFF0C\u7528\u4E8E\uFF1A\uFF081\uFF09\u8282\u70B9\u53D1\u73B0\uFF08mDNS\u5728\u516C\u7F51\u4E0D\u53EF\u7528\uFF09\uFF1B\uFF082\uFF09\u6D88\u606F\u4E2D\u8F6C\uFF08TURN\u4E2D\u7EE7\uFF09\uFF1B\uFF083\uFF09\u79BB\u7EBF\u961F\u5217\u540C\u6B65\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.2 \u5B89\u5168\u5C42\u8865\u5145")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u5EFA\u8BAE3: Skill\u6743\u9650\u7CFB\u7EDF")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u5B9E\u73B0\u7C7B\u4F3C\u4E8EAndroid\u7684\u6743\u9650\u7CFB\u7EDF\uFF0C\u5728Skill\u5B89\u88C5\u65F6\u663E\u793A\u8BF7\u6C42\u7684\u6743\u9650\uFF0C\u7528\u6237\u53EF\u4EE5\u9009\u62E9\u6027\u6388\u6743\uFF1A")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "INTERNET", bold: true }), new TextRun(" - \u7F51\u7EDC\u8BBF\u95EE")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "FILE_READ/WRITE", bold: true }), new TextRun(" - \u6587\u4EF6\u7CFB\u7EDF\u8BBF\u95EE")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "EXEC_COMMANDS", bold: true }), new TextRun(" - \u7CFB\u7EDF\u547D\u4EE4\u6267\u884C")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "ENV_ACCESS", bold: true }), new TextRun(" - \u73AF\u5883\u53D8\u91CF\u8BBF\u95EE")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "MEMORY_ACCESS", bold: true }), new TextRun(" - CIS\u8BB0\u5FC6\u8BBF\u95EE")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.3 \u5F00\u53D1\u8BA1\u5212\u8C03\u6574")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u5EFA\u8BAE\u7684\u4FEE\u8BA2\u5F00\u53D1\u8BA1\u5212")] }),
      new Table({
        columnWidths: [3120, 2340, 2340, 1560],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u9636\u6BB5", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u539F\u4F30\u7B97", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5EFA\u8BAE\u8C03\u6574", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u539F\u56E0", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Phase 1: Skill\u57FA\u7840\u8BBE\u65BD")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("2\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "3\u5468", color: colors.warning })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u8FB9\u7F18\u60C5\u51B5")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Phase 2: CIS\u6A21\u5757\u96C6\u6210")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("2\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("2\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5408\u7406")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Phase 3: \u6D88\u606F\u6E20\u9053\u5C42")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("0\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "4-5\u5468", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u672A\u8003\u8651")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Phase 4: \u5F31\u7F51\u573A\u666F\u652F\u6301")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("0\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "2-3\u5468", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u672A\u8003\u8651")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Phase 5: \u6D4B\u8BD5\u4E0E\u6587\u6863")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("2\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("2\u5468")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5408\u7406")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ children: [new TextRun({ text: "\u603B\u8BA1", bold: true })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "6\u5468", bold: true })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "13-15\u5468", bold: true, color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, children: [new Paragraph({ children: [new TextRun("")] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 3: \u5F00\u53D1\u8BA1\u5212\u8C03\u6574\u5EFA\u8BAE", size: 20, color: colors.secondary })] }),
      
      // Risk Assessment
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u56DB\u3001\u98CE\u9669\u8BC4\u4F30\u8865\u5145")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("4.1 \u672A\u8BC6\u522B\u7684\u98CE\u9669")] }),
      new Table({
        columnWidths: [2340, 1560, 2340, 3120],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u98CE\u9669", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u7B49\u7EA7", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5F71\u54CD", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u7F13\u89E3\u63AA\u65BD", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Skill\u6076\u610F\u4EE3\u7801")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u9AD8", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u7CFB\u7EDF\u5165\u4FB5\u3001\u6570\u636E\u6CC4\u9732")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6743\u9650\u7CFB\u7EDF + WASM\u6C99\u7BB1")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5F00\u53D1\u5468\u671F\u5EF6\u8BEF")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u9AD8", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u9879\u76EE\u5EF6\u8FDF\u3001\u6210\u672C\u8D85\u652F")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5206\u9636\u6BB5\u4EA4\u4ED8 + \u7F13\u51B2\u65F6\u95F4")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5E73\u53F0API\u53D8\u66F4")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4E2D", color: colors.warning })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6E20\u9053\u5C42\u5931\u6548")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u62BD\u8C61\u5C42 + \u591A\u5E73\u53F0SDK")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("OpenClaw\u751F\u6001\u53D8\u5316")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4E2D", color: colors.warning })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Skill\u683C\u5F0F\u4E0D\u517C\u5BB9")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u7248\u672C\u9501\u5B9A + \u517C\u5BB9\u5C42")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u8BB8\u53EF\u8BC1\u5408\u89C4\u98CE\u9669")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 1560, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u4E2D", color: colors.warning })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u6CD5\u5F8B\u7EA0\u7EB7")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u8BB8\u53EF\u8BC1\u58F0\u660E + \u7528\u6237\u786E\u8BA4")] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 4: \u8865\u5145\u98CE\u9669\u8BC4\u4F30", size: 20, color: colors.secondary })] }),
      
      // Final Conclusion
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E94\u3001\u603B\u7ED3\u4E0E\u5EFA\u8BAE")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("5.1 \u4F01\u5212\u8BC4\u4EF7")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("Kimi\u6574\u5408\u7684\u4F01\u5212\u6587\u6863"), new TextRun({ text: "\u8D28\u91CF\u8F83\u9AD8", bold: true, color: colors.success }), new TextRun("\uFF0C\u6280\u672F\u5206\u6790\u6DF1\u5165\uFF0C\u65B9\u6848\u8BBE\u8BA1\u5408\u7406\u3002\u4E3B\u8981\u4F18\u70B9\u5305\u62EC\uFF1A")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun("\u6B63\u786E\u9009\u62E9\u4E86\u201C\u76F4\u63A5\u63A5\u5165Skill\u201D\u65B9\u6848\uFF0C\u907F\u514D\u4E86Gateway\u96C6\u6210\u7684\u590D\u6742\u6027")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun("\u8BE6\u7EC6\u7684\u6280\u672F\u5B9E\u73B0\u65B9\u6848\uFF0C\u5305\u542B\u4EE3\u7801\u793A\u4F8B")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun("\u8003\u8651\u4E86\u5F00\u6E90\u8D23\u4EFB\u98CE\u9669\uFF0C\u63D0\u51FA\u4E86\u8BB8\u53EF\u8BC1\u58F0\u660E\u65B9\u6845")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, spacing: { after: 200 }, children: [new TextRun("\u529F\u80FD\u8986\u76D6\u5206\u6790\u6E05\u6670\uFF0C\u660E\u786E\u6307\u51FA\u9700\u8981\u5F00\u53D1\u7684\u90E8\u5206")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("5.2 \u4E3B\u8981\u4E0D\u8DB3")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "\u5DE5\u4F5C\u91CF\u4F30\u7B97\u8FC7\u4E8E\u4E50\u89C2", color: colors.danger }), new TextRun(" - \u5B9E\u9645\u9700\u898113-15\u5468\uFF0C\u800C\u975E4-6\u5468")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "\u672A\u8003\u8651\u5F31\u7F51\u573A\u666F", color: colors.danger }), new TextRun(" - \u8FD9\u662F\u6570\u5B57\u6E38\u6C11\u7684\u6838\u5FC3\u9700\u6C42")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "Skill\u5B89\u5168\u8FB9\u754C\u4E0D\u6E05\u6670", color: colors.warning }), new TextRun(" - \u9700\u8981\u66F4\u7EC6\u7C92\u5EA6\u7684\u6743\u9650\u63A7\u5236")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u6D88\u606F\u6E20\u9053\u5C42\u590D\u6742\u5EA6\u4F4E\u4F30", color: colors.warning }), new TextRun(" - \u6BCF\u4E2A\u5E73\u53F0\u90FD\u6709\u5176\u7279\u6709\u590D\u6742\u6027")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("5.3 \u6700\u7EC8\u5EFA\u8BAE")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u63A8\u8350\u5B9E\u65BD", bold: true, color: colors.success }), new TextRun("\uFF0C\u4F46\u9700\u8981\u8C03\u6574\u8BA1\u5212\u548C\u589E\u52A0\u5173\u952E\u6A21\u5757\u3002\u5EFA\u8BAE\u5206\u4E09\u4E2A\u9636\u6BB5\u5B9E\u65BD\uFF1A")] }),
      new Paragraph({ numbering: { reference: "numbered-4", level: 0 }, children: [new TextRun({ text: "MVP\u9636\u6BB5\uFF084-5\u5468\uFF09\uFF1A", bold: true }), new TextRun("Skill\u89E3\u6790\u5668 + \u57FA\u7840\u5DE5\u5177\u6620\u5C04 + \u4E0ESummarize\u7B49\u7B80\u5355Skill\u96C6\u6210")] }),
      new Paragraph({ numbering: { reference: "numbered-4", level: 0 }, children: [new TextRun({ text: "\u6E20\u9053\u9636\u6BB5\uFF084-5\u5468\uFF09\uFF1A", bold: true }), new TextRun("Telegram/Discord\u6E20\u9053 + \u57FA\u7840IM\u8DEF\u7531")] }),
      new Paragraph({ numbering: { reference: "numbered-4", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u5F31\u7F51\u9636\u6BB5\uFF082-3\u5468\uFF09\uFF1A", bold: true }), new TextRun("\u79BB\u7EBF\u961F\u5217 + \u4E91\u7AEF\u4FE1\u4EE4 + \u5F02\u6784\u4EFB\u52A1\u8DEF\u7531")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u603B\u4F30\u7B97\uFF1A", bold: true }), new TextRun("10-13\u5468\uFF0C\u6BD4\u539F\u4F01\u5212\u4F30\u7B97\u591A2\u5468\uFF0C\u4F46\u6BD4Gateway\u96C6\u6210\u65B9\u6848\uFF0811\u5468\uFF09\u4ECD\u6709\u4F18\u52BF\uFF0C\u4E14\u529F\u80FD\u66F4\u5B8C\u5584\u3002")] })
    ]
  }]
});

Packer.toBuffer(doc).then(buffer => {
  fs.writeFileSync("/home/z/my-project/download/CIS_OpenClaw_企划评判报告.docx", buffer);
  console.log("Report generated: /home/z/my-project/download/CIS_OpenClaw_企划评判报告.docx");
});
