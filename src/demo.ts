export const projectName = '雾港来信';

type Chapter = {
  file: string;
  label: string;
  title: string;
  paragraphs: readonly string[];
};

export const chapters: readonly Chapter[] = [
  {
    file: '00-front-matter.md',
    label: '卷首',
    title: projectName,
    paragraphs: ['献给每一个在退潮时回家的人。'],
  },
  {
    file: '01-return-to-the-harbor.md',
    label: '第一章',
    title: '归港',
    paragraphs: [
      '雾是清晨涨起来的。等渡船靠岸，远处的灯塔已经隐去了塔身，只剩一星灯火，悬在潮水与天色之间。林迟站在舷梯旁，数着岸上那一排系船的铁桩。第七根还是歪的。',
      '邮差没来。她收起那封没有落款的信，沿石阶往镇上走。信纸薄得透光，隔着外套口袋，她也能摸到一道折痕。那是父亲惯用的折法：先折短边，再把尖角向里收。',
      '码头边卖鱼的人正把木箱拖过湿漉漉的路面。有人抬头瞧了她一眼，又低下去。七年前她离开雾港的时候，也是这样的天气。那一天，所有人都知道她要走。',
      '只有等她走到旧邮局门口，才发现那盏悬在檐下的绿灯还亮着。玻璃门后有一小块擦得很干净的柜台，上面放着一只杯子，正慢慢冒出热气。',
    ],
  },
  {
    file: '02-the-post-office.md',
    label: '第二章',
    title: '旧邮局',
    paragraphs: [
      '门没有锁。铜铃响过一声，屋里又静下来。柜台后面是排到天花板的信格，沿着墙的弧度转过去，最深处没在暗里。',
      '林迟走到窗下。桌上摊着一本账簿，最后一行的墨还没干。寄件人一栏写着她的名字，日期却是明天。',
    ],
  },
  {
    file: '03-the-tide-ledger.md',
    label: '第三章',
    title: '潮汐簿',
    paragraphs: [
      '镇上的人从不用钟表算退潮。潮汐簿每天在鱼市开张前挂上公告栏，谁也没见过落笔的人。',
      '那天的簿子多出一页。林迟把铜钉拔下来时，太阳在水面上露了片刻。她看见纸背印着一枚浅蓝色的邮戳。',
    ],
  },
];

type Prompt = {
  file: string;
  title: string;
  body: string;
};

export const writingPrompts: readonly Prompt[] = [
  {
    file: 'prompts/writing/continuity.md',
    title: '章节连贯性检查',
    body: '核对当前章节与前一章的时间、地点和人物状态。逐条指出矛盾，附上两处对应段落和理由。保留所有悬念，仅在文本互相冲突时提出修改建议。',
  },
  {
    file: 'prompts/writing/chapter-summary.md',
    title: '章节摘要',
    body: '按发生顺序概括本章事件。单列人物新获得的信息、对前文的呼应和仍待解决的线索。对推测注明不确定性。',
  },
];

export const illustrationPrompts: readonly Prompt[] = [
  {
    file: 'prompts/illustration/01-harbor.md',
    title: '第一章 · 雾中的渡口',
    body: '清晨的雾港，湿润的石码头，一艘小渡船刚刚靠岸。远处雾中只有一点暖色灯火，前景是一根歪斜的系船铁桩。冷灰与旧黄铜色，低饱和，横幅构图。',
  },
  {
    file: 'prompts/illustration/02-post-office.md',
    title: '第二章 · 旧邮局',
    body: '老邮局的昏暗内景，木制信格转过一道弧形墙，纸页在窗边透出冷光。柜台上有一杯热水，屋内空无一人。细腻纸本插画，柔和颗粒。',
  },
];
