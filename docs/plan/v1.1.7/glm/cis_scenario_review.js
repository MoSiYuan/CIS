const { Document, Packer, Paragraph, TextRun, Table, TableRow, TableCell, 
        Header, Footer, AlignmentType, LevelFormat, HeadingLevel, BorderStyle, 
        WidthType, ShadingType, VerticalAlign, PageNumber, PageBreak } = require('docx');
const fs = require('fs');

// Color scheme - Wilderness Oasis
const colors = {
  primary: "1A1F16",
  body: "2D3329",
  secondary: "4A5548",
  accent: "94A3B8",
  tableBg: "F8FAF7",
  success: "16A34A",
  warning: "CA8A04",
  danger: "DC2626"
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
        children: [new TextRun({ text: "CIS \u67B6\u6784\u5BA1\u67E5 - \u6570\u5B57\u6E38\u6C11\u573A\u666F", font: "SimHei", size: 20, color: colors.secondary })]
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
      new Paragraph({ heading: HeadingLevel.TITLE, children: [new TextRun("CIS \u67B6\u6784\u5BA1\u67E5\u62A5\u544A")] }),
      new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 200 },
        children: [new TextRun({ text: "\u6570\u5B57\u6E38\u6C11\u5F31\u7F51\u5F02\u6784\u7F16\u8BD1\u573A\u666F\u9002\u914D\u6027\u5206\u6790", color: colors.secondary, size: 26 })] }),
      new Paragraph({ alignment: AlignmentType.CENTER, spacing: { after: 400 },
        children: [new TextRun({ text: "\u5BA1\u67E5\u65E5\u671F: 2026-02-16", color: colors.secondary, size: 22 })] }),
      
      // Scenario Description
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E00\u3001\u573A\u666F\u63CF\u8FF0")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u76EE\u6807\u573A\u666F\uFF1A", bold: true }), new TextRun("\u6E38\u620F\u5F00\u53D1\u4E2D\uFF0C\u9700\u8981 Mac Metal ARM \u67B6\u6784\u7F16\u8BD1\u3001Windows CUDA 5090 x64 \u67B6\u6784\u7F16\u8BD1\u3002\u5F00\u53D1\u8005\u5728\u5496\u5561\u9986\u7528 AI Agent \u5199\u4EE3\u7801\uFF0C\u901A\u8FC7\u5F31\u7F51\u4E91\u7AEF\u8282\u70B9\u4F5C\u4E3A mDNS \u6253\u901A\u81EA\u5DF1\u5F00\u53D1\u673A\u548C\u4F4F\u6240\u5F3A\u7F51\u673A\uFF0C\u5411\u5F3A\u7F51\u673A\u63A8\u9001 Git \u63D0\u4EA4\uFF0C\u5E76\u544A\u77E5\u4E24\u53F0\u5F02\u6784\u4E3B\u673A\u5F00\u59CB\u7F16\u8BD1\u81EA\u52A8\u6D4B\u8BD5\u3002")] }),
      
      // Scenario Analysis
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("1.1 \u573A\u666F\u8981\u7D20\u5206\u89E3")] }),
      new Table({
        columnWidths: [2340, 3120, 3900],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8981\u7D20", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u7279\u5F81", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u6280\u672F\u6311\u6218", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5F31\u7F51\u73AF\u5883")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5496\u5561\u9986WiFi\uFF0C\u4E0D\u7A33\u5B9A\u3001\u9AD8\u5EF6\u8FDF\u3001\u4F4E\u5E26\u5BBD")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u79BB\u7EBF\u961F\u5217\u3001\u65AD\u70B9\u7EED\u4F20\u3001\u6D88\u606F\u6301\u4E45\u5316")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u4E91\u7AEF\u4E2D\u7EE7")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u516C\u7F51\u8282\u70B9\u4F5C\u4E3A mDNS/NAT \u7A7F\u900F\u4E2D\u7EE7")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("TURN \u4E2D\u7EE7\u3001\u4FE1\u4EE4\u670D\u52A1\u3001DHT \u8DEF\u7531")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5F02\u6784\u7F16\u8BD1")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Mac Metal ARM + Windows CUDA x64")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u8282\u70B9\u80FD\u529B\u6807\u7B7E\u3001\u4EFB\u52A1\u8DEF\u7531\u3001\u7ED3\u679C\u805A\u5408")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Git \u96C6\u6210")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u63A8\u9001\u4EE3\u7801\u89E6\u53D1\u7F16\u8BD1\u6D4B\u8BD5")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Webhook \u63A5\u6536\u3001\u4E8B\u4EF6\u89E6\u53D1\u3001DAG \u8C03\u5EA6")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("AI Agent")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3120, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5F00\u53D1\u8005\u7528 AI \u5199\u4EE3\u7801")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 3900, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Agent Provider \u63A5\u53E3\u3001\u53CC\u5411\u8C03\u7528")] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 1: \u573A\u666F\u8981\u7D20\u5206\u89E3", size: 20, color: colors.secondary })] }),
      
      // Architecture Review
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E8C\u3001\u67B6\u6784\u5408\u7406\u6027\u5BA1\u67E5")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("2.1 \u73B0\u6709\u67B6\u6784\u5206\u6790")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("CIS \u91C7\u7528\u5206\u5C42\u67B6\u6784\uFF1A\u8868\u73B0\u5C42\uFF08GUI/CLI\uFF09\u2192 \u63A7\u5236\u5C42\uFF08DAG\u8C03\u5EA6\u5668\u3001Agent\u6267\u884C\u5668\uFF09\u2192 \u7269\u7406\u5C42\uFF08\u5B58\u50A8\u3001P2P\u7F51\u7EDC\uFF09\u3002\u8FD9\u79CD\u5206\u5C42\u8BBE\u8BA1\u4F7F\u5F97\u5404\u5C42\u804C\u8D23\u660E\u786E\uFF0C\u4FBF\u4E8E\u7EF4\u62A4\u548C\u6D4B\u8BD5\u3002\u6838\u5FC3\u5E93 cis-core \u5305\u542B 30+ \u4E2A\u5B50\u6A21\u5757\uFF0C\u901A\u8FC7 feature flags \u63A7\u5236\u529F\u80FD\u5F00\u5173\uFF0C\u652F\u6301\u6309\u9700\u7F16\u8BD1\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u4F18\u70B9")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "\u6A21\u5757\u5316\u7A0B\u5EA6\u9AD8\uFF1A", bold: true }), new TextRun("19 \u4E2A workspace \u6210\u5458\uFF0C\u804C\u8D23\u5206\u79BB\u6E05\u6670\uFF0C\u7B26\u5408 SOLID \u539F\u5219\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "P2P \u7F51\u7EDC\u5B8C\u5584\uFF1A", bold: true }), new TextRun("QUIC \u4F20\u8F93\u3001Kademlia DHT\u3001mDNS \u53D1\u73B0\u3001NAT \u7A7F\u900F\u3001Noise \u52A0\u5BC6\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, children: [new TextRun({ text: "CRDT \u540C\u6B65\uFF1A", bold: true }), new TextRun("LWW-Register\u3001Vector Clock\u3001OR-Set\uFF0C\u652F\u6301\u79BB\u7EBF\u5408\u5E76\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-1", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "Agent \u96C6\u6210\uFF1A", bold: true }), new TextRun("\u652F\u6301 Claude\u3001Kimi\u3001Aider \u7B49\u591A\u79CD Agent\uFF0C\u53CC\u5411\u8C03\u7528\u673A\u5236\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u4E0D\u8DB3")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun({ text: "\u7F3A\u5C11\u79BB\u7EBF\u961F\u5217\uFF1A", bold: true }), new TextRun("\u5F31\u7F51\u73AF\u5883\u4E0B\u6D88\u606F\u65E0\u6CD5\u6301\u4E45\u5316\uFF0C\u65AD\u7EBF\u540E\u4E22\u5931\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun({ text: "\u65E0\u5F02\u6784\u4EFB\u52A1\u8DEF\u7531\uFF1A", bold: true }), new TextRun("DAG \u8282\u70B9\u65E0\u6CD5\u6307\u5B9A\u7279\u5B9A\u8282\u70B9\u6267\u884C\uFF08\u5982 Mac \u7F16\u8BD1 vs Windows \u7F16\u8BD1\uFF09\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, children: [new TextRun({ text: "\u7F3A\u5C11\u65AD\u70B9\u7EED\u4F20\uFF1A", bold: true }), new TextRun("\u5927\u6587\u4EF6\u4F20\u8F93\u65E0\u6CD5\u4ECE\u4E2D\u65AD\u5904\u7EE7\u7EED\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-2", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u65E0\u5E26\u5BBD\u81EA\u9002\u5E94\uFF1A", bold: true }), new TextRun("\u5F31\u7F51\u73AF\u5883\u4E0B\u65E0\u6CD5\u81EA\u52A8\u964D\u4F4E\u540C\u6B65\u9891\u7387\u6216\u6570\u636E\u91CF\u3002")] }),
      
      // Weak Network Adaptation
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E09\u3001\u5F31\u7F51\u573A\u666F\u9002\u914D\u6027\u5206\u6790")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.1 \u73B0\u6709\u80FD\u529B\u8BC4\u4F30")] }),
      new Table({
        columnWidths: [2340, 2340, 2340, 2340],
        margins: { top: 100, bottom: 100, left: 180, right: 180 },
        rows: [
          new TableRow({
            tableHeader: true,
            children: [
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u529F\u80FD", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u72B6\u6001", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8BC4\u4EF7", bold: true, size: 22 })] })] }),
              new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, shading: { fill: colors.tableBg, type: ShadingType.CLEAR }, verticalAlign: VerticalAlign.CENTER,
                children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8BF4\u660E", bold: true, size: 22 })] })] })
            ]
          }),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("NAT \u7A7F\u900F")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5DF2\u5B9E\u73B0", color: colors.success })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u2605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("UPnP/STUN/TURN/Hole Punch")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("mDNS \u53D1\u73B0")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5DF2\u5B9E\u73B0", color: colors.success })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u2605")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5C40\u57DF\u7F51\u8282\u70B9\u53D1\u73B0")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("CRDT \u540C\u6B65")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u5DF2\u5B9E\u73B0", color: colors.success })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u2605\u2605\u2605\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u79BB\u7EBF\u5408\u5E76\uFF0C\u4F46\u7F3A\u5C11\u961F\u5217")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u79BB\u7EBF\u961F\u5217")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u672A\u5B9E\u73B0", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u25CB\u25CB\u25CB\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5F31\u7F51\u573A\u666F\u5173\u952E\u7F3A\u5931")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5F02\u6784\u4EFB\u52A1\u8DEF\u7531")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u672A\u5B9E\u73B0", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u25CB\u25CB\u25CB\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u65E0\u6CD5\u6307\u5B9A\u8282\u70B9\u6267\u884C\u7279\u5B9A\u4EFB\u52A1")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u65AD\u70B9\u7EED\u4F20")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u672A\u5B9E\u73B0", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u25CB\u25CB\u25CB\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u5927\u6587\u4EF6\u4F20\u8F93\u65E0\u6CD5\u7EED\u4F20")] })] })
          ]}),
          new TableRow({ children: [
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("Git \u96C6\u6210")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u672A\u5B9E\u73B0", color: colors.danger })] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ alignment: AlignmentType.CENTER, children: [new TextRun("\u2605\u25CB\u25CB\u25CB\u25CB")] })] }),
            new TableCell({ borders: cellBorders, width: { size: 2340, type: WidthType.DXA }, children: [new Paragraph({ children: [new TextRun("\u65E0 Webhook \u6216 Git \u4E8B\u4EF6\u76D1\u542C")] })] })
          ]})
        ]
      }),
      new Paragraph({ spacing: { before: 200, after: 200 }, alignment: AlignmentType.CENTER, children: [new TextRun({ text: "\u8868 2: \u5F31\u7F51\u573A\u666F\u80FD\u529B\u8BC4\u4F30", size: 20, color: colors.secondary })] }),
      
      // Scenario Workflow Analysis
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("3.2 \u573A\u666F\u5DE5\u4F5C\u6D41\u5206\u6790")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("\u4EE5\u4E0B\u5206\u6790\u5728\u5F53\u524D CIS \u67B6\u6784\u4E0B\uFF0C\u8BE5\u573A\u666F\u7684\u6267\u884C\u6D41\u7A0B\u53CA\u5176\u95EE\u9898\uFF1A")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u6B65\u9AA4 1: \u5496\u5561\u9986\u8FDE\u63A5\u4E91\u7AEF\u8282\u70B9")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u73B0\u72B6\uFF1A", bold: true }), new TextRun("NAT \u7A7F\u900F\u6A21\u5757\u652F\u6301 UPnP\u3001STUN\u3001TURN\uFF0C\u53EF\u4EE5\u901A\u8FC7\u4E91\u7AEF TURN \u4E2D\u7EE7\u5EFA\u7ACB\u8FDE\u63A5\u3002mDNS \u6A21\u5757\u53EF\u4EE5\u5728\u5C40\u57DF\u7F51\u53D1\u73B0\u8282\u70B9\u3002"), new TextRun({ text: "\u95EE\u9898\uFF1A", bold: true, color: colors.danger }), new TextRun("\u5496\u5561\u9986 WiFi \u53EF\u80FD\u662F\u5BF9\u79F0 NAT\uFF0C\u9700\u8981 TURN \u4E2D\u7EE7\uFF0C\u4F46 TURN \u670D\u52A1\u5668\u5217\u8868\u4E3A\u7A7A\uFF08DEFAULT_TURN_SERVERS \u4E3A\u7A7A\u6570\u7EC4\uFF09\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u6B65\u9AA4 2: AI Agent \u5199\u4EE3\u7801\u5E76\u63A8\u9001 Git")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u73B0\u72B6\uFF1A", bold: true }), new TextRun("Agent Provider \u63A5\u53E3\u652F\u6301 Claude\u3001Kimi \u7B49\uFF0C\u53EF\u4EE5\u8C03\u7528 AI \u5199\u4EE3\u7801\u3002"), new TextRun({ text: "\u95EE\u9898\uFF1A", bold: true, color: colors.danger }), new TextRun("CIS \u6CA1\u6709 Git \u96C6\u6210\uFF0C\u65E0\u6CD5\u81EA\u52A8\u76D1\u542C Git \u4E8B\u4EF6\u6216\u6267\u884C git push\u3002\u9700\u8981\u5F00\u53D1\u8005\u624B\u52A8\u6267\u884C git push\uFF0C\u7136\u540E\u901A\u8FC7 CIS CLI \u624B\u52A8\u89E6\u53D1\u7F16\u8BD1\u4EFB\u52A1\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u6B65\u9AA4 3: \u901A\u77E5\u5F02\u6784\u4E3B\u673A\u5F00\u59CB\u7F16\u8BD1")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u73B0\u72B6\uFF1A", bold: true }), new TextRun("DAG \u6267\u884C\u5668\u53EF\u4EE5\u6267\u884C\u4EFB\u52A1\u4F9D\u8D56\u56FE\u3002"), new TextRun({ text: "\u95EE\u9898\uFF1A", bold: true, color: colors.danger }), new TextRun("DAG \u8282\u70B9\u6CA1\u6709\u201C\u8282\u70B9\u9009\u62E9\u5668\u201D\u6216\u201C\u80FD\u529B\u6807\u7B7E\u201D\uFF0C\u65E0\u6CD5\u6307\u5B9A\u201C\u6B64\u4EFB\u52A1\u5FC5\u987B\u5728 Mac \u4E0A\u6267\u884C\u201D\u6216\u201C\u6B64\u4EFB\u52A1\u5FC5\u987B\u5728 Windows \u4E0A\u6267\u884C\u201D\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_3, children: [new TextRun("\u6B65\u9AA4 4: \u7F16\u8BD1\u7ED3\u679C\u56DE\u4F20")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u73B0\u72B6\uFF1A", bold: true }), new TextRun("CRDT \u540C\u6B65\u53EF\u4EE5\u5408\u5E76\u516C\u57DF\u8BB0\u5FC6\uFF0C\u7406\u8BBA\u4E0A\u53EF\u4EE5\u540C\u6B65\u7F16\u8BD1\u7ED3\u679C\u3002"), new TextRun({ text: "\u95EE\u9898\uFF1A", bold: true, color: colors.danger }), new TextRun("\u7F16\u8BD1\u4EA7\u7269\uFF08\u4E8C\u8FDB\u5236\u6587\u4EF6\u3001\u65E5\u5FD7\uFF09\u53EF\u80FD\u5F88\u5927\uFF0C\u5F53\u524D\u540C\u6B65\u673A\u5236\u6CA1\u6709\u5206\u5757\u4F20\u8F93\u3001\u65AD\u70B9\u7EED\u4F20\u3001\u538B\u7F29\u7B49\u4F18\u5316\u3002")] }),
      
      // Improvement Suggestions
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u56DB\u3001\u6539\u8FDB\u5EFA\u8BAE")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("4.1 \u9AD8\u4F18\u5148\u7EA7\uFF08\u573A\u666F\u5FC5\u9700\uFF09")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "\u5B9E\u73B0\u79BB\u7EBF\u6D88\u606F\u961F\u5217\uFF1A", bold: true })] }),
      new Paragraph({ indent: { left: 720 }, spacing: { after: 100 },
        children: [new TextRun("\u5728\u672C\u5730\u6301\u4E45\u5316\u5F85\u53D1\u9001\u7684\u6D88\u606F\uFF0C\u7F51\u7EDC\u6062\u590D\u540E\u81EA\u52A8\u91CD\u8BD5\u3002\u4F7F\u7528 SQLite \u5B58\u50A8\u961F\u5217\u72B6\u6001\uFF0C\u786E\u4FDD\u4E0D\u4E22\u5931\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, children: [new TextRun({ text: "\u6DFB\u52A0\u8282\u70B9\u80FD\u529B\u6807\u7B7E\u548C\u4EFB\u52A1\u8DEF\u7531\uFF1A", bold: true })] }),
      new Paragraph({ indent: { left: 720 }, spacing: { after: 100 },
        children: [new TextRun("DAG \u8282\u70B9\u6DFB\u52A0 \u201Cnode_selector\u201D \u5B57\u6BB5\uFF0C\u652F\u6301\u6307\u5B9A\u8282\u70B9\u5FC5\u987B\u6EE1\u8DB3\u7684\u6761\u4EF6\uFF08\u5982 os=macos, arch=arm64, gpu=metal\uFF09\u3002\u8282\u70B9\u6CE8\u518C\u65F6\u4E0A\u62A5\u81EA\u5DF1\u7684\u80FD\u529B\u6807\u7B7E\u3002")] }),
      new Paragraph({ numbering: { reference: "numbered-3", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u6DFB\u52A0 TURN \u670D\u52A1\u5668\u914D\u7F6E\uFF1A", bold: true })] }),
      new Paragraph({ indent: { left: 720 }, spacing: { after: 200 },
        children: [new TextRun("\u9ED8\u8BA4 TURN \u670D\u52A1\u5668\u5217\u8868\u4E3A\u7A7A\uFF0C\u9700\u8981\u63D0\u4F9B\u9ED8\u8BA4\u516C\u5171 TURN \u670D\u52A1\u5668\u6216\u5141\u8BB8\u7528\u6237\u914D\u7F6E\u81EA\u5DF1\u7684 TURN \u670D\u52A1\u5668\u3002")] }),
      
      new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun("4.2 \u4E2D\u4F18\u5148\u7EA7\uFF08\u4F53\u9A8C\u4F18\u5316\uFF09")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "Git \u96C6\u6210\uFF1A", bold: true }), new TextRun("\u6DFB\u52A0 Git Webhook \u63A5\u6536\u5668\uFF0C\u81EA\u52A8\u76D1\u542C push \u4E8B\u4EF6\u5E76\u89E6\u53D1 DAG \u6267\u884C\u3002")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "\u65AD\u70B9\u7EED\u4F20\uFF1A", bold: true }), new TextRun("\u5927\u6587\u4EF6\u4F20\u8F93\u652F\u6301\u5206\u5757\u4F20\u8F93\u548C\u6821\u9A8C\u70B9\uFF0C\u4E2D\u65AD\u540E\u53EF\u4ECE\u6700\u540E\u4E00\u4E2A\u6210\u529F\u5757\u7EE7\u7EED\u3002")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "\u5E26\u5BBD\u81EA\u9002\u5E94\uFF1A", bold: true }), new TextRun("\u68C0\u6D4B\u7F51\u7EDC\u8D28\u91CF\uFF0C\u81EA\u52A8\u8C03\u6574\u540C\u6B65\u9891\u7387\u548C\u6570\u636E\u538B\u7F29\u7EA7\u522B\u3002")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u4E91\u7AEF\u4FE1\u4EE4\u670D\u52A1\uFF1A", bold: true }), new TextRun("\u63D0\u4F9B\u4E00\u4E2A\u516C\u7F51\u53EF\u8BBF\u95EE\u7684\u4FE1\u4EE4\u8282\u70B9\uFF0C\u7528\u4E8E\u5F31\u7F51\u73AF\u5883\u4E0B\u7684\u8282\u70B9\u53D1\u73B0\u548C\u6D88\u606F\u4E2D\u8F6C\u3002")] }),
      
      // Conclusion
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u4E94\u3001\u603B\u7ED3")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun("CIS \u5728 P2P \u7F51\u7EDC\u3001NAT \u7A7F\u900F\u3001CRDT \u540C\u6B65\u7B49\u57FA\u7840\u8BBE\u65BD\u65B9\u9762\u5DF2\u7ECF\u6709\u826F\u597D\u7684\u57FA\u7840\uFF0C\u4F46\u5BF9\u4E8E\u201C\u6570\u5B57\u6E38\u6C11\u5F31\u7F51\u5F02\u6784\u7F16\u8BD1\u201D\u8FD9\u4E00\u7279\u5B9A\u573A\u666F\uFF0C\u8FD8\u7F3A\u5C11\u4EE5\u4E0B\u5173\u952E\u80FD\u529B\uFF1A")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "\u79BB\u7EBF\u6D88\u606F\u961F\u5217", bold: true }), new TextRun(" - \u5F31\u7F51\u573A\u666F\u7684\u6838\u5FC3\u4FDD\u969C")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "\u5F02\u6784\u4EFB\u52A1\u8DEF\u7531", bold: true }), new TextRun(" - \u6307\u5B9A\u7279\u5B9A\u8282\u70B9\u6267\u884C\u7279\u5B9A\u4EFB\u52A1")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, children: [new TextRun({ text: "Git \u96C6\u6210", bold: true }), new TextRun(" - \u81EA\u52A8\u5316\u89E6\u53D1\u7F16\u8BD1\u6D41\u7A0B")] }),
      new Paragraph({ numbering: { reference: "bullet-list", level: 0 }, spacing: { after: 200 }, children: [new TextRun({ text: "\u4E91\u7AEF\u4FE1\u4EE4\u670D\u52A1", bold: true }), new TextRun(" - \u516C\u7F51\u8282\u70B9\u4E2D\u7EE7")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u5EFA\u8BAE\uFF1A", bold: true }), new TextRun("\u5982\u679C\u8981\u652F\u6301\u8FD9\u4E2A\u573A\u666F\uFF0C\u9700\u8981\u5728\u73B0\u6709\u67B6\u6784\u4E0A\u589E\u52A0\u4E00\u5C42\u201C\u4EFB\u52A1\u8C03\u5EA6\u4E2D\u95F4\u4EF6\u201D\uFF0C\u8D1F\u8D23\u79BB\u7EBF\u961F\u5217\u7BA1\u7406\u3001\u8282\u70B9\u80FD\u529B\u5339\u914D\u3001\u4EFB\u52A1\u72B6\u6001\u8DDF\u8E2A\u7B49\u529F\u80FD\u3002\u540C\u65F6\u9700\u8981\u63D0\u4F9B\u4E00\u4E2A\u516C\u7F51\u53EF\u8BBF\u95EE\u7684\u4FE1\u4EE4\u8282\u70B9\uFF0C\u7528\u4E8E\u5F31\u7F51\u73AF\u5883\u4E0B\u7684\u8282\u70B9\u53D1\u73B0\u3002")] }),
      
      // Architecture Diagram
      new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun("\u9644\u5F55\uFF1A\u5EFA\u8BAE\u67B6\u6784\u56FE")] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u5F53\u524D\u67B6\u6784\uFF1A", bold: true })] }),
      new Paragraph({ spacing: { after: 200 },
        children: [new TextRun({ text: "\u5496\u5561\u9986\u7B14\u8BB0\u672C \u2190\u2192 [\u4E91\u7AEF TURN] \u2190\u2192 \u4F4F\u6240 Mac/Windows", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ indent: { firstLine: 480 }, spacing: { after: 200 },
        children: [new TextRun({ text: "\u5EFA\u8BAE\u6539\u8FDB\u67B6\u6784\uFF1A", bold: true })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "\u5496\u5561\u9986\u7B14\u8BB0\u672C", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "    \u2193 [\u79BB\u7EBF\u961F\u5217] \u2191 [\u81EA\u52A8\u540C\u6B65]", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "\u4E91\u7AEF\u4FE1\u4EE4\u8282\u70B9 (DHT + \u6D88\u606F\u961F\u5217)", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 100 },
        children: [new TextRun({ text: "    \u2193 [\u4EFB\u52A1\u8DEF\u7531]", font: "SarasaMonoSC", size: 20 })] }),
      new Paragraph({ spacing: { after: 200 },
        children: [new TextRun({ text: "\u4F4F\u6240 Mac [os=macos,arch=arm64,gpu=metal]  |  \u4F4F\u6240 Windows [os=windows,arch=x64,gpu=cuda]", font: "SarasaMonoSC", size: 20 })] })
    ]
  }]
});

Packer.toBuffer(doc).then(buffer => {
  fs.writeFileSync("/home/z/my-project/download/CIS_Scenario_Review_Report.docx", buffer);
  console.log("Report generated: /home/z/my-project/download/CIS_Scenario_Review_Report.docx");
});
