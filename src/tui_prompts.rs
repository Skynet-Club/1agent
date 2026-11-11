//! Prompt supplements shown in the `/` command drawer.
//!
//! Source: the user-provided Feishu prompt table (48 entries, imported
//! 2026-11-11), embedded as static data so the drawer works offline and never
//! depends on the network. Re-import when the table changes.
//!
//! Chinese is the primary language. `PROMPT_ENGLISH` carries the subset whose
//! instruction text exists verbatim in the source table; English mode shows
//! only those entries rather than silently falling back to Chinese.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptSupplement {
    pub id: &'static str,
    pub name: &'static str,
    pub category: &'static str,
    pub stage: &'static str,
    pub value: &'static str,
    pub purpose: &'static str,
    pub when: &'static str,
    pub not_when: &'static str,
    pub body: &'static str,
    pub variables: &'static str,
    pub insertion: &'static str,
    pub output: &'static str,
    pub source: &'static str,
}

pub const PROMPT_SUPPLEMENTS: &[PromptSupplement] = &[
    PromptSupplement {
        id: "prompt-01",
        name: "真正的目的是",
        category: "加目标",
        stage: "想清楚",
        value: "A 必放",
        purpose: "把「做这件事」换成「要这个结果」",
        when: "输入只有一个动作/对象，没有说为了什么",
        not_when: "用户已经说清目的时，会显得啰嗦",
        body: "我真正想要的不是这件事本身，而是{{结果}}。请先判断：做这件事是不是达成结果的必要一步？如果不是，告诉我该做什么。",
        variables: "结果是什么",
        insertion: "追加在末尾",
        output: "对话",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-02",
        name: "下一步拿它干什么",
        category: "加目标",
        stage: "想清楚",
        value: "A 必放",
        purpose: "让模型按下游用途的规格产出",
        when: "输入没说产出给谁用、下一步是什么",
        not_when: "纯探索、还没有下游时",
        body: "做完这个之后，我下一步要拿它干什么。请按那个下游用途的要求来产出，而不是按这个任务本身的常见格式。",
        variables: "下一步用途",
        insertion: "追加在末尾",
        output: "对话",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-03",
        name: "先说验收标准",
        category: "加目标",
        stage: "想清楚",
        value: "A 必放",
        purpose: "动手前先定义「好」长什么样",
        when: "任务复杂、标准模糊、容易返工",
        not_when: "心里已经有明确标准时",
        body: "先别动手。用三句话写出这个任务「做得好」的验收标准，等我确认之后再开始做。",
        variables: "",
        insertion: "作为前置指令",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-04",
        name: "往上抬一层",
        category: "加目标",
        stage: "想清楚",
        value: "A- 推荐",
        purpose: "从任务层跳到问题层",
        when: "用户给的方案明显是手段不是目的",
        not_when: "任务本身就是目的（如交作业）",
        body: "别在这个任务的层面回答。先说出这个任务存在的真正原因，然后给我一个比它更有效的方案。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-05",
        name: "失败预演",
        category: "加目标",
        stage: "做检查",
        value: "A 必放",
        purpose: "从失败倒推该做什么",
        when: "要投入钱或时间的决定",
        not_when: "低风险小事",
        body: "如果我{{时间}}后失败了，最可能的原因是什么？先列三个，再据此调整你的方案。",
        variables: "时间（如三个月）",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-06",
        name: "用专家真会用的工具",
        category: "加深度",
        stage: "做方案",
        value: "A 必放",
        purpose: "从「像专家说」变成「像专家做」",
        when: "答案开始流于表面、只有泛泛之谈",
        not_when: "闲聊、创意发想",
        body: "不要用表格和我能自己想到的常识。请用这个领域顶尖从业者实际会用的方法和工具（数据查询、统计模型、实验设计、现场核查），并说明每一步为什么用它。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-07",
        name: "先要原始数据",
        category: "加深度",
        stage: "找信息",
        value: "A 必放",
        purpose: "不让模型基于转述下结论",
        when: "问题涉及判断、而你只有印象没有数据",
        not_when: "纯创意/主观偏好问题",
        body: "不要基于我的描述下结论。先列出你需要哪些原始数据，缺的标出来，再基于现有的部分给结论。",
        variables: "",
        insertion: "作为前置指令",
        output: "清单",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-08",
        name: "定不可能的目标",
        category: "加深度",
        stage: "做方案",
        value: "A- 推荐",
        purpose: "用一个不合理的数字逼出新路径",
        when: "目标定得温吞、改进只有百分之几",
        not_when: "受硬性物理/法规限制的目标",
        body: "把目标定为现在的 1/{{倍数}}（具体：{{数字}}）。它看起来不合理，但请当作完全可解，先给出三条能达成它的路径。",
        variables: "倍数、具体数字",
        insertion: "追加在末尾",
        output: "报告",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-09",
        name: "挑战我的假设",
        category: "加深度",
        stage: "做检查",
        value: "A 必放",
        purpose: "让模型对抗你而不是顺着你",
        when: "你带着一个判断来提问",
        not_when: "假设已经被验证过",
        body: "我认为{{假设}}。请用你能拿到的信息检验它；证据不足就直说，不要顺着我。",
        variables: "假设",
        insertion: "作为前置指令",
        output: "报告",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-10",
        name: "产出新知识",
        category: "加深度",
        stage: "做方案",
        value: "A- 推荐",
        purpose: "要求它发现你不知道的东西",
        when: "已经拿到一批数据或资料",
        not_when: "信息量太少时",
        body: "在你已经掌握的信息里，找出一个我还没意识到的模式，并说明它为什么能在新数据上成立。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-11",
        name: "找外部裁判",
        category: "加深度",
        stage: "做检查",
        value: "A 必放",
        purpose: "不让模型给自己打分",
        when: "需要判断方案对错",
        not_when: "主观审美类任务",
        body: "不要自己评价自己的答案。列出三个可以检验它的外部标准（可查的数据、权威来源、可运行的测试），并按它们逐条打分，低于及格线就重做。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-12",
        name: "换领域看",
        category: "加广度",
        stage: "做方案",
        value: "A- 推荐",
        purpose: "借另一个行业的框架",
        when: "方案陷入同类竞品的常规做法",
        not_when: "领域完全不相关时",
        body: "用{{另一个领域}}的框架重新看这件事，指出那个领域会怎么做、哪里会做得不一样。",
        variables: "另一个领域",
        insertion: "追加在末尾",
        output: "对话",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-13",
        name: "名词换动词",
        category: "加广度",
        stage: "想清楚",
        value: "A 必放",
        purpose: "把抽象名词拆成具体行为",
        when: "输入里全是名词（方案/体系/打法）",
        not_when: "技术规格类问题",
        body: "把我刚才那句话里的名词都换成动词（例如「方案」→「让谁在什么条件下做出什么决定」），然后按动词版本重做。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-14",
        name: "组队",
        category: "加广度",
        stage: "做方案",
        value: "A- 推荐",
        purpose: "引入多个视角并行",
        when: "问题复杂、单一视角容易漏",
        not_when: "小事、单人可完成",
        body: "如果这件事要一个三人小队来做，你会找哪三个角色？分别让他们做他们各自会做的事（不是模仿他们的口吻），最后汇总成一份。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "Felix Rieseberg",
    },
    PromptSupplement {
        id: "prompt-15",
        name: "反方最强论证",
        category: "加广度",
        stage: "做检查",
        value: "A 必放",
        purpose: "先听完最强的反对再决定",
        when: "你要做一个自己已经倾向的决定",
        not_when: "已经拍板、只需执行",
        body: "请站在最可能反对这件事的人的立场，写出他最强的三条反驳，然后逐条回应。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-16",
        name: "先给结论",
        category: "加约束",
        stage: "做交付",
        value: "A 必放",
        purpose: "压掉铺垫",
        when: "模型爱先讲背景再给答案",
        not_when: "需要铺垫情绪的写作",
        body: "第一句话就给结论（能/不能、是/不是、该/不该），再展开理由。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-17",
        name: "字数与例子硬约束",
        category: "加约束",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "逼它具体",
        when: "输出又长又空",
        not_when: "需要铺陈的长文",
        body: "全文不超过 {{N}} 字。不许用形容词堆砌，每个论断后面跟一个具体例子或数字。",
        variables: "N 字",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-18",
        name: "禁用词清单",
        category: "加约束",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "去掉黑话与腔调",
        when: "输出像行业通稿",
        not_when: "需要行业术语的专业文档",
        body: "不要用这些词：赋能、闭环、生态、抓手、打法、护城河。不要用排比句和感叹号。违反任何一条就重写。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-19",
        name: "格式锁定",
        category: "加约束",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "让输出可解析可贴表",
        when: "要拿去填系统或表格",
        not_when: "需要解释判断依据时",
        body: "只输出一个 Markdown 表格，列固定为：{{列1}} | {{列2}} | {{列3}}。表格外不许有任何文字。",
        variables: "列名",
        insertion: "追加在末尾",
        output: "表格",
        source: "Riley Goodside",
    },
    PromptSupplement {
        id: "prompt-20",
        name: "给外行讲",
        category: "加约束",
        stage: "做交付",
        value: "A 必放",
        purpose: "把术语换成生活里的东西",
        when: "要给不懂的人看",
        not_when: "专业读者",
        body: "用我奶奶能听懂的话讲一遍：不许用术语，每个术语都要换成一个生活里的东西或一个具体场景。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-21",
        name: "每个数字标来源",
        category: "加证据",
        stage: "找信息",
        value: "A 必放",
        purpose: "让结论可核",
        when: "涉及价格、政策、门槛",
        not_when: "纯观点讨论",
        body: "每个关键数字后面标来源（官方页 / 论文 / 第三方整理），拿不到就写「无法核实」，不要估算。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-22",
        name: "可信度分三档",
        category: "加证据",
        stage: "找信息",
        value: "A 必放",
        purpose: "把事实和推测分开",
        when: "模型把推测混在事实里",
        not_when: "已有充分实证时",
        body: "把结论按证据强度分成三档：官方原文 / 行业口径 / 我的推测。推测部分单独列在最后。",
        variables: "",
        insertion: "追加在末尾",
        output: "表格",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-23",
        name: "先查再答",
        category: "加证据",
        stage: "找信息",
        value: "A- 推荐",
        purpose: "强制检索而不是回忆",
        when: "问的是会变的现状",
        not_when: "通用知识问答",
        body: "先说明你要查什么、查到了什么，再回答。查不到的明确说查不到。",
        variables: "",
        insertion: "作为前置指令",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-24",
        name: "反例搜索",
        category: "加证据",
        stage: "做检查",
        value: "A- 推荐",
        purpose: "检验结论的适用范围",
        when: "结论听起来太顺",
        not_when: "无法搜证的未来预测",
        body: "给我三个和这个结论相反的真实案例。如果找不到，说明这个结论的适用范围有限。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-25",
        name: "标注核验日期",
        category: "加证据",
        stage: "找信息",
        value: "A 必放",
        purpose: "防止过期信息",
        when: "引用政策/费率/版本号",
        not_when: "稳定的常识",
        body: "所有价格、政策、版本号后面标注核验日期，并在结尾列出哪些会随时间变化。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-26",
        name: "缺数据就留空",
        category: "加证据",
        stage: "找信息",
        value: "A 必放",
        purpose: "禁止估算补位",
        when: "表格里会出现空缺",
        not_when: "需要快速给个数量级时",
        body: "数据缺失的地方留空写「—」，严禁用估算或行业平均值补位。",
        variables: "",
        insertion: "追加在末尾",
        output: "表格",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-27",
        name: "自我攻击",
        category: "加检验",
        stage: "做检查",
        value: "A 必放",
        purpose: "找出自己方案的死穴",
        when: "方案已经成型",
        not_when: "方案还在发散期",
        body: "现在把它当作别人的方案。找出最可能出错的三个地方，按影响大小排序，每个给一个补救动作。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-28",
        name: "预演失败复盘",
        category: "加检验",
        stage: "做检查",
        value: "A 必放",
        purpose: "提前写失败报告",
        when: "重大投入前",
        not_when: "已经在做了、只想推进时",
        body: "假设三个月后这件事失败了。写一份复盘：哪一步最先出问题、当时的信号是什么、我应该在哪一天停下来。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-29",
        name: "红队审查",
        category: "加检验",
        stage: "做检查",
        value: "A- 推荐",
        purpose: "逐条质问假设",
        when: "要给别人的方案做审",
        not_when: "团队情绪脆弱时",
        body: "扮演一个专门挑刺的审查者，逐条质问我方案里的每个假设，不许客气，也不许为了让我舒服而软化。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-30",
        name: "最小可证伪实验",
        category: "加检验",
        stage: "做检查",
        value: "A 必放",
        purpose: "用一周证伪最贵的一环",
        when: "不确定但不想等",
        not_when: "已经充分验证过",
        body: "这个方案里最不确定的一环是什么？设计一个一周内能做完、能证明我错了的最小实验，给出判断标准和预算上限。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-31",
        name: "两次答案",
        category: "加检验",
        stage: "做检查",
        value: "A- 推荐",
        purpose: "用第二版校准第一版",
        when: "第一版答案来得太快",
        not_when: "时间紧张时",
        body: "先给你的第一版答案。然后假设第一版是错的，给出第二版，并说明两版的差别在哪、哪一版更可能是对的。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-32",
        name: "给老板看",
        category: "加读者",
        stage: "做交付",
        value: "A 必放",
        purpose: "改成决策格式",
        when: "输出要向上汇报",
        not_when: "自己看的笔记",
        body: "改写成给老板看的版本：第一句结论，接着三行依据，最后是要他拍板的那一件事。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-33",
        name: "给执行的人看",
        category: "加读者",
        stage: "做交付",
        value: "A 必放",
        purpose: "改成可操作格式",
        when: "输出要给团队做",
        not_when: "给决策层看时",
        body: "改写成给一线执行的版本：每一步做什么、谁做、要多久、交付物是什么。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-34",
        name: "给顾客/家长看",
        category: "加读者",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "从对方感受出发",
        when: "要写对外文案",
        not_when: "内部复盘",
        body: "改写成给{{对象}}看的版本：只讲他能感受到的变化，不讲我们的流程和体系。",
        variables: "对象（顾客/家长/藏家）",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-35",
        name: "给三个月后的自己",
        category: "加读者",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "写成可接续的笔记",
        when: "任务会中断",
        not_when: "一次性的小问题",
        body: "写成三个月后我自己能看懂、能直接接着做的笔记：结论、依据、未决问题、下一步动作。",
        variables: "",
        insertion: "追加在末尾",
        output: "报告",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-36",
        name: "变成可勾选清单",
        category: "加执行",
        stage: "做交付",
        value: "A 必放",
        purpose: "把结论变成待办",
        when: "方案要落地",
        not_when: "还在讨论方向时",
        body: "把结论变成一份可勾选的清单，每条都能判断「做完/没做完」。不要写「加强」「优化」「提升」这类无法验收的词。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-37",
        name: "排出关键路径",
        category: "加执行",
        stage: "做方案",
        value: "A 必放",
        purpose: "找出决定开工时间的那几件",
        when: "事项很多、不知先做哪件",
        not_when: "事情很少时",
        body: "这件事里哪三件决定了什么时候能开始？其余能不能并行？按这个排一张时间表。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-38",
        name: "每条给验收方式",
        category: "加执行",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "让产出自带质检",
        when: "产出要交给别人",
        not_when: "纯创意产出",
        body: "每一条产出后面写出验收方式：谁、看什么、什么算通过。",
        variables: "",
        insertion: "追加在末尾",
        output: "表格",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-39",
        name: "钱、时间、人力三笔账",
        category: "加执行",
        stage: "做方案",
        value: "A- 推荐",
        purpose: "算清成本再动手",
        when: "要投钱",
        not_when: "几乎零成本的尝试",
        body: "列出这件事的钱、时间、人力三笔账，并给出「最省」和「最稳」两个版本。",
        variables: "",
        insertion: "追加在末尾",
        output: "表格",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-40",
        name: "只做第一件",
        category: "加执行",
        stage: "做交付",
        value: "A 必放",
        purpose: "砍到单一动作",
        when: "拖延、无法启动",
        not_when: "需要并行推进时",
        body: "如果只能先做一件事，做哪件？给出这一件事的具体动作和完成时间，其余全部先放下。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-41",
        name: "画出来",
        category: "加形态",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "用图替代文字解释",
        when: "流程、结构、关系复杂",
        not_when: "简单线性步骤",
        body: "不要用文字解释。先列出所有节点和它们之间的关系，再用 Mermaid 图表示。",
        variables: "",
        insertion: "追加在末尾",
        output: "图",
        source: "Karpathy",
    },
    PromptSupplement {
        id: "prompt-42",
        name: "做成网页",
        category: "加形态",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "变成能点开的东西",
        when: "要给人演示或试用",
        not_when: "最终产物不是给人看的",
        body: "直接输出一个单文件 HTML，能交互、能点开看细节，不要解释代码。",
        variables: "",
        insertion: "追加在末尾",
        output: "代码",
        source: "Karpathy",
    },
    PromptSupplement {
        id: "prompt-43",
        name: "做成对比表",
        category: "加形态",
        stage: "做交付",
        value: "A 必放",
        purpose: "一眼可比",
        when: "多个选项要比较",
        not_when: "只有一个选项时",
        body: "把内容做成一张对比表：行是{{对象}}，列是{{维度}}，不适用的格子写「—」。",
        variables: "对象、维度",
        insertion: "追加在末尾",
        output: "表格",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-44",
        name: "写成脚本",
        category: "加形态",
        stage: "做交付",
        value: "B 备用",
        purpose: "变成可执行的步骤",
        when: "流程要重复做",
        not_when: "一次性的手工活",
        body: "把流程写成一个可执行的脚本或命令序列，每一步带一行注释说明它在验证什么。",
        variables: "",
        insertion: "追加在末尾",
        output: "代码",
        source: "Karpathy",
    },
    PromptSupplement {
        id: "prompt-45",
        name: "30 分钟版本",
        category: "加时间",
        stage: "做方案",
        value: "A 必放",
        purpose: "先要粗版再看方向",
        when: "方向未定、怕白做",
        not_when: "标准已明确、要终稿",
        body: "先给一个 30 分钟内能做完的粗版本，我确认方向后再深做。不要追求完整。",
        variables: "",
        insertion: "追加在末尾",
        output: "对话",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-46",
        name: "本周能做的",
        category: "加时间",
        stage: "做交付",
        value: "A- 推荐",
        purpose: "把方案压到可执行粒度",
        when: "方案太大、落不了地",
        not_when: "长周期规划期",
        body: "把方案压缩成这周能落地的三件事，其余的排到后面并注明依赖什么。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-47",
        name: "倒推时间表",
        category: "加时间",
        stage: "做方案",
        value: "B 备用",
        purpose: "从截止日反推",
        when: "有明确 deadline",
        not_when: "没有截止日时",
        body: "从{{截止日}}倒推，列出每周该完成什么，标出不能延后的关键几步。",
        variables: "截止日",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
    PromptSupplement {
        id: "prompt-48",
        name: "先做最便宜的验证",
        category: "加时间",
        stage: "做方案",
        value: "A- 推荐",
        purpose: "把不可逆的动作推到最后",
        when: "要花大钱但不确定",
        not_when: "成本都很低时",
        body: "哪一步最贵或最不可逆？先设计一个便宜的替代验证，把它推迟到最后。",
        variables: "",
        insertion: "追加在末尾",
        output: "清单",
        source: "自研",
    },
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptLanguage {
    #[default]
    Zh,
    En,
}

impl PromptLanguage {
    pub fn label(self) -> &'static str {
        match self {
            Self::Zh => "中文",
            Self::En => "English",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "zh" | "cn" | "中文" => Some(Self::Zh),
            "en" | "english" | "英文" => Some(Self::En),
            _ => None,
        }
    }
}

/// English instruction text for cards whose source table row contains a
/// verbatim English original. Cards without this entry are hidden in English
/// mode.
pub struct PromptEnglish {
    pub name: &'static str,
    pub purpose: &'static str,
    pub when: &'static str,
    pub not_when: &'static str,
    pub body: &'static str,
    pub variables: &'static str,
    pub source_records: &'static str,
    pub source_nature: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromptView {
    pub name: &'static str,
    pub purpose: &'static str,
    pub when: &'static str,
    pub not_when: &'static str,
    pub body: &'static str,
    pub variables: &'static str,
    pub category: &'static str,
    pub value: &'static str,
}

pub const PROMPT_ENGLISH: &[(&str, PromptEnglish)] = &[
    (
        "prompt-01",
        PromptEnglish {
            name: "The real goal",
            purpose: "Replace the action with the outcome you actually want",
            when: "The input names only an action or object, not the purpose",
            not_when: "The user has already stated the purpose",
            body: "Altitude: Going up in abstraction and considering the outcome rather than the individual task. Ask yourself: what do you want to happen next? What's the goal beyond the goal?",
            variables: "",
            source_records: "reczz28YEz7bqXfd, reczz28YEz7bnzet",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-02",
        PromptEnglish {
            name: "What happens next",
            purpose: "Produce to the spec of the downstream use",
            when: "The input does not say who consumes the output or what comes next",
            not_when: "Pure exploration with no downstream use yet",
            body: "What do you want to happen next? What's the goal beyond the goal? Produce to the requirements of that downstream use, not the task's default format.",
            variables: "",
            source_records: "reczz28YEz7bnzet",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-04",
        PromptEnglish {
            name: "Go up a level",
            purpose: "Move from the task layer to the problem layer",
            when: "The user's proposal is a means, not the goal",
            not_when: "The task itself is the goal (for example, homework)",
            body: "Altitude: Going up in abstraction and considering the outcome rather than the individual task.",
            variables: "",
            source_records: "reczz28YEz7bqXfd",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-06",
        PromptEnglish {
            name: "Use the instruments experts use",
            purpose: "Move from sounding like an expert to working like one",
            when: "The answer is surface-level and full of generic advice",
            not_when: "Small talk or idea generation",
            body: "Insist on the same instruments used by the best in the industry: When prompted to act like a professional, models often switch vocabulary without actually switching methods.",
            variables: "",
            source_records: "reczz28YEz7bp7hu",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-07",
        PromptEnglish {
            name: "Ask for the raw data",
            purpose: "Don't let the model conclude from someone else's summary",
            when: "The user is describing the situation second-hand",
            not_when: "The raw data is unavailable or not needed",
            body: "Challenge what you know: Especially in business, a lot of things we believe to be true are rough approximations of reality. Give the model access to the raw information and data and ask it to adversarially check assumptions.",
            variables: "",
            source_records: "reczz28YEz7bon7v",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-08",
        PromptEnglish {
            name: "Set an unreasonable goal",
            purpose: "Force a new path with an unreasonable number",
            when: "Incremental improvements are not enough",
            not_when: "The goal must be a hard constraint",
            body: "Pick unreasonable goals and tell the model that they're very reasonable. You need your website to load in 5s instead of 10s? Ask for 200ms. Does something seem impossible to the model? Reiterate that it's very much solvable, it just needs to keep going.",
            variables: "",
            source_records: "reczz28YEz7bpWV6, reczz28YEz7bpCa3",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-09",
        PromptEnglish {
            name: "Challenge my assumptions",
            purpose: "Test beliefs against evidence instead of agreeing",
            when: "The user states a strong belief as fact",
            not_when: "The belief is already verified or low-stakes",
            body: "Challenge what you know: A lot of things we believe to be true are rough approximations of reality. Adversarially check my assumptions against the raw data; if the evidence is insufficient, say so instead of agreeing.",
            variables: "",
            source_records: "reczz28YEz7bon7v",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-10",
        PromptEnglish {
            name: "Produce new knowledge",
            purpose: "Find a pattern the user has not noticed",
            when: "The task is analysis or research, not just summarization",
            not_when: "The user only wants existing conclusions",
            body: "Produce new knowledge: Build a simulation of our customers that reproduces the last year from our logs. Then find the earliest signal we don't know about yet, and show it predicts on data it has not seen.",
            variables: "",
            source_records: "reczz28YEz7bvSxs",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-11",
        PromptEnglish {
            name: "Use an outside judge",
            purpose: "Don't let the model grade its own work",
            when: "The result needs an independent check",
            not_when: "No external check is available",
            body: "Use an outside judge: Do not ask the model to judge itself.",
            variables: "",
            source_records: "reczz28YEz7budNf",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-12",
        PromptEnglish {
            name: "Borrow another field",
            purpose: "Use frameworks from outside the task's surface field",
            when: "The task is stuck in one discipline",
            not_when: "The task requires strict domain compliance",
            body: "Range: Drawing on multiple fields rather than just the one the task might need at the surface, using knowledge, tools, and techniques you normally wouldn't.",
            variables: "",
            source_records: "reczz28YEz7bsLhg",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-13",
        PromptEnglish {
            name: "Nouns into verbs",
            purpose: "Turn a deliverable into the outcome it must produce",
            when: "The request names a document or artifact instead of an outcome",
            not_when: "The artifact itself is the required deliverable",
            body: "Try to replace the nouns with verbs: A quarterly review isn't \"a deck\". It's \"getting busy people to believe a story about cause and effect, and to decide under uncertainty, in 30 minutes\".",
            variables: "",
            source_records: "reczz28YEz7bvhDp",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-14",
        PromptEnglish {
            name: "Assemble the crew",
            purpose: "Use the methods of several roles, not their voices",
            when: "The task needs multiple kinds of expertise",
            not_when: "A single expert is enough",
            body: "Assemble the crew: Use this method to find fields and techniques. Don't ask the model to write like a short seller, ask it to do what a short seller does to research their position.",
            variables: "",
            source_records: "reczz28YEz7btdT1",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-41",
        PromptEnglish {
            name: "Draw it",
            purpose: "Replace prose with a diagram",
            when: "The content is structural or relational",
            not_when: "The output must be prose only",
            body: "Diagrams / images. Instead of writing, ask your LLM to create a diagram. These can be a lot easier to process, parse, and understand.",
            variables: "",
            source_records: "reczz28YFWXElhp4",
            source_nature: "本人原文",
        },
    ),
    (
        "prompt-42",
        PromptEnglish {
            name: "Make a web page",
            purpose: "Turn the answer into an interactive single-file page",
            when: "The answer benefits from interaction or visual layout",
            not_when: "A plain text answer is enough",
            body: "Web pages. Ask for output \"in HTML\" to get a beautiful, interactive webpage. LLMs are getting really good at frontend and can create beautiful experiences, animations, etc.",
            variables: "",
            source_records: "reczz28YFWXEkdit",
            source_nature: "本人原文",
        },
    ),
];

pub fn prompt_english(id: &str) -> Option<&'static PromptEnglish> {
    PROMPT_ENGLISH
        .iter()
        .find(|(card_id, _)| *card_id == id)
        .map(|(_, english)| english)
}

pub fn prompt_view(
    prompt: &'static PromptSupplement,
    language: PromptLanguage,
) -> Option<PromptView> {
    match language {
        PromptLanguage::Zh => Some(PromptView {
            name: prompt.name,
            purpose: prompt.purpose,
            when: prompt.when,
            not_when: prompt.not_when,
            body: prompt.body,
            variables: prompt.variables,
            category: prompt.category,
            value: prompt.value,
        }),
        PromptLanguage::En => {
            let english = prompt_english(prompt.id)?;
            Some(PromptView {
                name: english.name,
                purpose: english.purpose,
                when: english.when,
                not_when: english.not_when,
                body: english.body,
                variables: english.variables,
                category: category_label_en(prompt.category),
                value: value_label_en(prompt.value),
            })
        }
    }
}

fn category_label_en(category: &str) -> &'static str {
    match category {
        "加目标" => "Goal",
        "加深度" => "Depth",
        "加证据" => "Evidence",
        "加约束" => "Constraints",
        "加检验" => "Verification",
        "加执行" => "Execution",
        "加读者" => "Audience",
        "加时间" => "Time",
        "加形态" => "Format",
        "加广度" => "Breadth",
        _ => "Prompt",
    }
}

fn value_label_en(value: &'static str) -> &'static str {
    match value {
        "A 必放" => "A Essential",
        "A- 推荐" => "A- Recommended",
        "B 备用" => "B Optional",
        _ => value,
    }
}

/// Empty-query drawer rows: a small, high-value starting set. The rest stay
/// searchable by name, category, trigger or purpose.
pub const FEATURED_PROMPTS: &[&str] = &[
    "prompt-01", // 真正的目的是
    "prompt-03", // 先说验收标准
    "prompt-16", // 先给结论
    "prompt-21", // 每个数字标来源
    "prompt-09", // 挑战我的假设
    "prompt-32", // 给老板看
    "prompt-36", // 变成可勾选清单
    "prompt-45", // 30 分钟版本
];

pub fn prompt_score(prompt: &PromptSupplement, query: &str) -> Option<u8> {
    let query = query.trim_start_matches('/').to_lowercase();
    if query.is_empty() {
        return Some(0);
    }
    let name = prompt.name.to_lowercase();
    let english = prompt_english(prompt.id);
    // Two-letter ASCII queries such as `st` are command prefixes, not a
    // useful English prompt search; require at least three characters.
    let search_english = query.chars().count() >= 3;
    let english_name = english
        .filter(|_| search_english)
        .map(|english| english.name.to_lowercase());
    if name == query || english_name.as_ref() == Some(&query) {
        Some(0)
    } else if name.starts_with(&query)
        || english_name
            .as_ref()
            .is_some_and(|name| name.starts_with(&query))
    {
        Some(1)
    } else if name.contains(&query)
        || english_name
            .as_ref()
            .is_some_and(|name| name.contains(&query))
    {
        Some(2)
    } else if (query.chars().count() >= 2 || !query.is_ascii())
        && (prompt.category.to_lowercase().contains(&query)
            || prompt.purpose.to_lowercase().contains(&query)
            || prompt.when.to_lowercase().contains(&query)
            || prompt.body.to_lowercase().contains(&query)
            || (search_english
                && english.is_some_and(|english| {
                    english.purpose.to_lowercase().contains(&query)
                        || english.when.to_lowercase().contains(&query)
                        || english.body.to_lowercase().contains(&query)
                })))
    {
        Some(3)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_keeps_all_entries_and_featured_ids_are_valid() {
        assert_eq!(PROMPT_SUPPLEMENTS.len(), 48);
        assert_eq!(
            PROMPT_SUPPLEMENTS
                .iter()
                .filter(|prompt| prompt.value == "A 必放")
                .count(),
            26
        );
        for id in FEATURED_PROMPTS {
            assert!(
                PROMPT_SUPPLEMENTS.iter().any(|prompt| prompt.id == *id),
                "missing featured prompt {id}"
            );
        }
        for prompt in PROMPT_SUPPLEMENTS {
            assert!(!prompt.name.is_empty());
            assert!(!prompt.body.is_empty());
        }
    }

    #[test]
    fn english_library_covers_only_cards_with_a_verbatim_source() {
        assert_eq!(PROMPT_ENGLISH.len(), 14);
        for (id, english) in PROMPT_ENGLISH {
            assert!(
                PROMPT_SUPPLEMENTS.iter().any(|prompt| prompt.id == *id),
                "missing card {id}"
            );
            assert!(!english.name.is_empty());
            assert!(!english.body.is_empty());
            assert!(!english.source_records.is_empty());
            assert_eq!(english.source_nature, "本人原文");
        }
        assert!(prompt_english("prompt-01").is_some());
        assert!(prompt_english("prompt-03").is_none());
        assert!(
            prompt_view(
                PROMPT_SUPPLEMENTS
                    .iter()
                    .find(|prompt| prompt.id == "prompt-01")
                    .unwrap(),
                PromptLanguage::En
            )
            .is_some()
        );
        assert!(
            prompt_view(
                PROMPT_SUPPLEMENTS
                    .iter()
                    .find(|prompt| prompt.id == "prompt-03")
                    .unwrap(),
                PromptLanguage::En
            )
            .is_none()
        );
    }

    #[test]
    fn search_matches_english_names_and_bodies() {
        let prompt = PROMPT_SUPPLEMENTS
            .iter()
            .find(|prompt| prompt.id == "prompt-01")
            .unwrap();
        assert_eq!(prompt_score(prompt, "The real goal"), Some(0));
        assert!(prompt_score(prompt, "abstraction").is_some());
    }

    #[test]
    fn search_matches_name_category_and_trigger() {
        let acceptance = PROMPT_SUPPLEMENTS
            .iter()
            .filter(|prompt| prompt_score(prompt, "/验收").is_some())
            .collect::<Vec<_>>();
        assert!(
            acceptance
                .iter()
                .any(|prompt| prompt.name == "先说验收标准")
        );
        assert!(
            PROMPT_SUPPLEMENTS.iter().any(
                |prompt| prompt.category == "加证据" && prompt_score(prompt, "/证据").is_some()
            )
        );
    }
}
