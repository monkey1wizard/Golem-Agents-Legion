# OpenCLI Coverage Snapshot

This document is a local snapshot of the currently installed OpenCLI registry on this machine.
Use it when you already know OpenCLI is the right lane and need a quick command inventory without rerunning `opencli list`.

- Snapshot source: `opencli list`
- Intended role: local lookup cache for GAL docs, not the upstream source of truth
- Drift risk: this file can become stale when OpenCLI is upgraded or local adapters change
- Refresh command: `opencli list`

For routing decisions, start with [opencli-routing.md](opencli-routing.md).
For the upstream catalog, see <https://github.com/jackwener/OpenCLI/blob/main/docs/adapters/index.md>.

## Current Local Snapshot

```text
  opencli — available commands

  1688
    assets [cookie] — 列出 1688 商品页可提取的图片/视频素材
    download [cookie] — 批量下载 1688 商品页可提取的图片和视频素材
    item [cookie] — 1688 商品详情（公开商品字段、价格阶梯、卖家基础信息）
    search [cookie] — 1688 商品搜索（结果候选、卖家链接、价格/MOQ/销量文本）
    store [cookie] — 1688 店铺/供应商公开信息（联系方式、主营、入驻年限、公开服务信号）

  36kr
    article [intercept] — 获取36氪文章正文内容
    hot [public] — 36氪热榜 — trending articles (renqi/zonghe/shoucang/catalog)
    news [public] — Latest tech/startup news from 36kr (36氪)
    search [public] — 搜索36氪文章

  amazon
    bestsellers [cookie] — Amazon Best Sellers pages for category candidate discovery
    discussion [cookie] — Amazon review summary and sample customer discussion from product review pages
    movers-shakers [cookie] — Amazon Movers & Shakers pages for short-term growth signals
    new-releases [cookie] — Amazon New Releases pages for early momentum discovery
    offer [cookie] — Amazon seller, buy box, and fulfillment facts from the product page
    product [cookie] — Amazon product page facts for candidate validation
    search [cookie] — Amazon search results for product discovery and coarse filtering

  antigravity
    dump [ui] — Dump the DOM to help AI understand the UI
    extract-code [ui] — Extract multi-line code blocks from the current Antigravity conversation
    model [ui] — Switch the active LLM model in Antigravity
    new [ui] — Start a new conversation / clear context in Antigravity
    read [ui] — Read the latest chat messages from Antigravity AI
    send [ui] — Send a message to Antigravity AI via the internal Lexical editor
    status [ui] — Check Antigravity CDP connection and get current page state
    watch [ui] — Stream new chat messages from Antigravity in real-time

  apple-podcasts
    episodes [public] — List recent episodes of an Apple Podcast (use ID from search)
    search [public] — Search Apple Podcasts
    top [public] — Top podcasts chart on Apple Podcasts

  arxiv
    paper [public] — Get arXiv paper details by ID
    search [public] — Search arXiv papers

  band
    bands [cookie] — List all Bands you belong to
    mentions [intercept] — Show Band notifications where you are @mentioned
    post [cookie] — Export full content of a post including comments
    posts [cookie] — List posts from a Band

  barchart
    flow [cookie] — Barchart unusual options activity / options flow
    greeks [cookie] — Barchart options greeks overview (IV, delta, gamma, theta, vega)
    options [cookie] — Barchart options chain with greeks, IV, volume, and open interest
    quote [cookie] — Barchart stock quote with price, volume, and key metrics

  bbc
    news [public] — BBC News headlines (RSS)

  bilibili
    comments [cookie] — 获取 B站视频评论（使用官方 API + WBI 签名）
    download [cookie] — 下载B站视频（需要 yt-dlp）
    dynamic [cookie] — Get Bilibili user dynamic feed
    favorite [cookie] — 我的默认收藏夹
    feed [cookie] — 关注的人的动态时间线
    following [cookie] — 获取 Bilibili 用户的关注列表
    history [cookie] — 我的观看历史
    hot [cookie] — B站热门视频
    me [cookie] — My Bilibili profile info
    ranking [cookie] — Get Bilibili video ranking board
    search [cookie] — Search Bilibili videos or users
    subtitle [cookie] — 获取 Bilibili 视频的字幕
    user-videos [cookie] — 查看指定用户的投稿视频

  bloomberg
    businessweek [public] — Bloomberg Businessweek top stories (RSS)
    economics [public] — Bloomberg Economics top stories (RSS)
    feeds [public] — List the Bloomberg RSS feed aliases used by the adapter
    industries [public] — Bloomberg Industries top stories (RSS)
    main [public] — Bloomberg homepage top stories (RSS)
    markets [public] — Bloomberg Markets top stories (RSS)
    news [cookie] — Read a Bloomberg story/article page and return title, full content, and media links
    opinions [public] — Bloomberg Opinion top stories (RSS)
    politics [public] — Bloomberg Politics top stories (RSS)
    tech [public] — Bloomberg Tech top stories (RSS)

  bluesky
    feeds [public] — Popular Bluesky feed generators
    followers [public] — List followers of a Bluesky user
    following [public] — List accounts a Bluesky user is following
    profile [public] — Get Bluesky user profile info
    search [public] — Search Bluesky users
    starter-packs [public] — Get starter packs created by a Bluesky user
    thread [public] — Get a Bluesky post thread with replies
    trending [public] — Trending topics on Bluesky
    user [public] — Get recent posts from a Bluesky user

  boss
    batchgreet [cookie] — BOSS直聘批量向推荐候选人发送招呼
    chatlist [cookie] — BOSS直聘查看聊天列表（招聘端）
    chatmsg [cookie] — BOSS直聘查看与候选人的聊天消息
    detail [cookie] — BOSS直聘查看职位详情
    exchange [cookie] — BOSS直聘交换联系方式（请求手机/微信）
    greet [cookie] — BOSS直聘向新候选人发送招呼（开始聊天）
    invite [cookie] — BOSS直聘发送面试邀请
    joblist [cookie] — BOSS直聘查看我发布的职位列表
    mark [cookie] — BOSS直聘给候选人添加标签
    recommend [cookie] — BOSS直聘查看推荐候选人（新招呼列表）
    resume [cookie] — BOSS直聘查看候选人简历（招聘端）
    search [cookie] — BOSS直聘搜索职位
    send [cookie] — BOSS直聘发送聊天消息
    stats [cookie] — BOSS直聘职位数据统计

  chaoxing
    assignments [cookie] — 学习通作业列表
    exams [cookie] — 学习通考试列表

  chatgpt
    ask [public] — Send a prompt and wait for the AI response (send + wait + read)
    model [public] — Switch ChatGPT Desktop model/mode (auto, instant, thinking, 5.2-instant, 5.2-thinking)
    new [public] — Open a new chat in ChatGPT Desktop App
    read [public] — Read the last visible message from the focused ChatGPT Desktop window
    send [public] — Send a message to the active ChatGPT Desktop App window
    status [public] — Check if ChatGPT Desktop App is running natively on macOS

  chatwise
    ask [ui] — Send a prompt and wait for the AI response (send + wait + read)
    export [ui] — Export the current ChatWise conversation to a Markdown file
    history [ui] — List conversation history in ChatWise sidebar
    model [ui] — Get or switch the active AI model in ChatWise
    read [ui] — Read the current ChatWise conversation history
    send [ui] — Send a message to the active ChatWise conversation

  cnki
    search [cookie] — 中国知网论文搜索（海外版）

  codex
    ask [ui] — Send a prompt and wait for the AI response (send + wait + read)
    export [ui] — Export the current Codex conversation to a Markdown file
    extract-diff [ui] — Extract visual code review diff patches from Codex
    history [ui] — List recent conversation threads in Codex
    model [ui] — Get or switch the currently active AI model in Codex Desktop
    read [ui] — Read the contents of the current Codex conversation thread
    send [ui] — Send text/commands to the Codex AI composer

  coupang
    add-to-cart [cookie] — Add a Coupang product to cart using logged-in browser session
    search [cookie] — Search Coupang products with logged-in browser session

  ctrip
    search [public] — 搜索携程目的地、景区和酒店联想结果

  cursor
    ask [ui] — Send a prompt and wait for the AI response (send + wait + read)
    composer [ui] — Send a prompt directly into Cursor Composer (Cmd+I shortcut)
    export [ui] — Export the current cursor conversation to a Markdown file
    extract-code [ui] — Extract multi-line code blocks from the current Cursor conversation
    history [ui] — List recent chat sessions from the Cursor sidebar
    model [ui] — Get or switch the currently active AI model in Cursor
    read [ui] — Read the current Cursor chat/composer conversation history
    send [ui] — Send a prompt directly into Cursor Composer/Chat

  devto
    tag [public] — Latest DEV.to articles for a specific tag
    top [public] — Top DEV.to articles of the day
    user [public] — Recent DEV.to articles from a specific user

  dictionary
    examples [public] — Read real-world example sentences utilizing the word
    search [public] — Search the Free Dictionary API for definitions, parts of speech, and pronunciations.
    synonyms [public] — Find synonyms for a specific word

  discord-app
    channels [ui] — List channels in the current Discord server
    members [ui] — List online members in the current Discord channel
    read [ui] — Read recent messages from the active Discord channel
    search [ui] — Search messages in the current Discord server/channel (Cmd+F)
    send [ui] — Send a message in the active Discord channel
    servers [ui] — List all Discord servers (guilds) in the sidebar
    status [ui] — Check active CDP connection to Discord Desktop

  douban
    book-hot [cookie] — 豆瓣图书热门榜单
    download [cookie] — 下载电影海报/剧照图片
    marks [cookie] — 导出个人观影标记
    movie-hot [cookie] — 豆瓣电影热门榜单
    photos [cookie] — 获取电影海报/剧照图片列表
    reviews [cookie] — 导出个人影评
    search [cookie] — 搜索豆瓣电影、图书或音乐
    subject [cookie] — 获取电影详情
    top250 [cookie] — 豆瓣电影 Top250

  doubao-app
    ask [ui] — Send a message to Doubao desktop app and wait for the AI response
    dump [ui] — Dump Doubao desktop app DOM and snapshot to /tmp for debugging
    new [ui] — Start a new chat in Doubao desktop app
    read [ui] — Read chat history from Doubao desktop app
    screenshot [ui] — Capture a screenshot of the Doubao desktop app window
    send [ui] — Send a message to Doubao desktop app
    status [ui] — Check CDP connection to Doubao desktop app

  doubao
    ask [cookie] — Send a prompt and wait for the Doubao response
    detail [cookie] — Read a specific Doubao conversation by ID
    history [cookie] — List conversation history from Doubao sidebar
    meeting-summary [cookie] — Get meeting summary and chapters from a Doubao conversation
    meeting-transcript [cookie] — Get or download the meeting transcript from a Doubao conversation
    new [cookie] — Start a new conversation in Doubao web chat
    read [cookie] — Read the current Doubao conversation history
    send [cookie] — Send a message to Doubao web chat
    status [cookie] — Check Doubao chat page availability and login state

  douyin
    activities [cookie] — 官方活动列表
    collections [cookie] — 合集列表
    delete [cookie] — 删除作品
    draft [cookie] — 上传视频并保存为草稿
    drafts [cookie] — 获取草稿列表
    hashtag [cookie] — 话题搜索 / AI推荐 / 热点词
    location [cookie] — 地理位置 POI 搜索
    profile [cookie] — 获取账号信息
    publish [cookie] — 定时发布视频到抖音（必须设置 2h ~ 14天后的发布时间）
    stats [cookie] — 作品数据分析
    update [cookie] — 更新视频信息
    user-videos [cookie] — 获取指定用户的视频列表（含下载地址和热门评论）
    videos [cookie] — 获取作品列表

  facebook
    add-friend [cookie] — Send a friend request on Facebook
    events [cookie] — Browse Facebook event categories
    feed [cookie] — Get your Facebook news feed
    friends [cookie] — Get Facebook friend suggestions
    groups [cookie] — List your Facebook groups
    join-group [cookie] — Join a Facebook group
    memories [cookie] — Get your Facebook memories (On This Day)
    notifications [cookie] — Get recent Facebook notifications
    profile [cookie] — Get Facebook user/page profile info
    search [cookie] — Search Facebook for people, pages, or posts

  gemini
    ask [cookie] — Send a prompt to Gemini and return only the assistant response
    deep-research [cookie] — Start a Gemini Deep Research run and confirm it
    deep-research-result [cookie] — Export Deep Research report URL from a Gemini conversation
    image [cookie] — Generate images with Gemini web and save them locally
    new [cookie] — Start a new conversation in Gemini web chat

  gitee
    search [public] — Search repositories on Gitee
    trending [public] — Recommended open-source projects on Gitee Explore
    user [public] — Show a Gitee user profile panel

  google
    news [public] — Get Google News headlines
    search [public] — Search Google
    suggest [public] — Get Google search suggestions
    trends [public] — Get Google Trends daily trending searches

  grok
    ask [cookie] — Send a message to Grok and get response

  hackernews
    ask [public] — Hacker News Ask HN posts
    best [public] — Hacker News best stories
    jobs [public] — Hacker News job postings
    new [public] — Hacker News newest stories
    search [public] — Search Hacker News stories
    show [public] — Hacker News Show HN posts
    top [public] — Hacker News top stories
    user [public] — Hacker News user profile

  hf
    top [public] — Top upvoted Hugging Face papers

  hupu
    detail [public] — 获取虎扑帖子详情 (使用Next.js JSON数据)
    hot [cookie] — 虎扑热门帖子
    like [cookie] — 点赞虎扑回复 (需要登录)
    mentions [cookie] (aliases: mention) — 查看虎扑提到我的回复 (需要登录)
    reply [cookie] — 回复虎扑帖子 (需要登录)
    search [public] — 搜索虎扑帖子 (使用官方API)
    unlike [cookie] — 取消点赞虎扑回复 (需要登录)

  imdb
    person [public] — Get actor or director info
    reviews [public] — Get user reviews for a movie or TV show
    search [public] — Search IMDb for movies, TV shows, and people
    title [public] — Get movie or TV show details
    top [public] — IMDb Top 250 Movies
    trending [public] — IMDb Most Popular Movies

  instagram
    comment [cookie] — Comment on an Instagram post
    download [cookie] — Download images and videos from Instagram posts and reels
    explore [cookie] — Instagram explore/discover trending posts
    follow [cookie] — Follow an Instagram user
    followers [cookie] — List followers of an Instagram user
    following [cookie] — List accounts an Instagram user is following
    like [cookie] — Like an Instagram post
    note [ui] — Publish a text Instagram note
    post [ui] — Post an Instagram feed image or mixed-media carousel
    profile [cookie] — Get Instagram user profile info
    reel [ui] — Post an Instagram reel video
    save [cookie] — Save (bookmark) an Instagram post
    saved [cookie] — Get your saved Instagram posts
    search [cookie] — Search Instagram users
    story [ui] — Post a single Instagram story image or video
    unfollow [cookie] — Unfollow an Instagram user
    unlike [cookie] — Unlike an Instagram post
    unsave [cookie] — Unsave (remove bookmark) an Instagram post
    user [cookie] — Get recent posts from an Instagram user

  jd
    add-cart [cookie] — 京东加入购物车
    cart [cookie] — 查看京东购物车
    detail [cookie] — 京东商品详情
    item [cookie] — 京东商品详情（价格、店铺、规格参数、AVIF 图片）
    reviews [cookie] — 京东商品评价
    search [cookie] — 京东商品搜索

  jianyu
    detail [cookie] — 读取剑鱼标讯详情页并抽取证据字段
    search [cookie] — 搜索剑鱼标讯公告

  jike
    comment [ui] — 评论即刻帖子
    create [ui] — 发布即刻动态
    feed [cookie] — 即刻首页动态流
    like [ui] — 点赞即刻帖子
    notifications [cookie] — 即刻通知
    post [cookie] — 即刻帖子详情及评论
    repost [ui] — 转发即刻帖子
    search [cookie] — 搜索即刻帖子
    topic [cookie] — 即刻话题/圈子帖子
    user [cookie] — 即刻用户动态

  jimeng
    generate [cookie] — 即梦AI 文生图 — 输入 prompt 生成图片
    history [cookie] — 即梦AI 查看最近生成的作品
    new [cookie] — 即梦AI 新建会话（workspace）
    workspaces [cookie] — 即梦AI 查看所有工作区（会话窗口）

  lesswrong
    comments [public] — Top comments on a post
    curated [public] — Curated editor's picks
    frontpage [public] — Algorithmic frontpage
    new [public] — Latest posts
    read [public] — Read full post by URL or ID
    sequences [public] — List post collections
    shortform [public] — Quick takes / shortform posts
    tag [public] — Posts by tag
    tags [public] — List popular tags
    top [public] — Top all-time
    top-month [public] — Top this month
    top-week [public] — Top this week
    top-year [public] — Top this year
    user [public] — User profile
    user-posts [public] — List a user's posts

  linkedin
    search [header] — Search LinkedIn jobs
    timeline [cookie] — Read LinkedIn home timeline posts

  linux-do
    categories [cookie] — linux.do 分类列表
    category [cookie] — linux.do 分类内话题
    feed [cookie] — linux.do 话题列表（需登录；支持全站、标签、分类）
    hot [cookie] — linux.do 热门话题
    latest [cookie] — linux.do 最新话题
    search [cookie] — 搜索 linux.do
    tags [cookie] — linux.do 标签列表
    topic [cookie] — linux.do 帖子首页摘要和回复（首屏）
    topic-content [cookie] — Get the main topic body as Markdown
    user-posts [cookie] — linux.do 用户的帖子
    user-topics [cookie] — linux.do 用户创建的话题

  lobsters
    active [public] — Lobste.rs most active discussions
    hot [public] — Lobste.rs hottest stories
    newest [public] — Lobste.rs newest stories
    tag [public] — Lobste.rs stories by tag

  medium
    feed [cookie] — Medium 热门文章 Feed
    search [cookie] — 搜索 Medium 文章
    user [cookie] — 获取 Medium 用户的文章列表

  notebooklm
    current [cookie] — Show metadata for the currently opened NotebookLM notebook tab
    get [cookie] (aliases: metadata) — Get rich metadata for the currently opened NotebookLM notebook
    history [cookie] — List NotebookLM conversation history threads in the current notebook
    list [cookie] — List NotebookLM notebooks via in-page batchexecute RPC in the current logged-in session
    note-list [cookie] (aliases: notes-list) — List saved notes from the Studio panel of the current NotebookLM notebook
    notes-get [cookie] — Get one note from the current NotebookLM notebook by title from the visible note editor
    open [cookie] (aliases: select) — Open one NotebookLM notebook in the automation workspace by id or URL
    source-fulltext [cookie] — Get the extracted fulltext for one source in the currently opened NotebookLM notebook
    source-get [cookie] — Get one source from the currently opened NotebookLM notebook by id or title
    source-guide [cookie] — Get the guide summary and keywords for one source in the currently opened NotebookLM notebook
    source-list [cookie] — List sources for the currently opened NotebookLM notebook
    status [cookie] — Check NotebookLM page availability and login state in the current Chrome session
    summary [cookie] — Get the summary block from the currently opened NotebookLM notebook

  notion
    export [ui] — Export the current Notion page as Markdown
    favorites [ui] — List pages from the Notion Favorites section in the sidebar
    new [ui] — Create a new page in Notion
    read [ui] — Read the content of the currently open Notion page
    search [ui] — Search pages and databases in Notion via Quick Find (Cmd+P)
    sidebar [ui] — List pages and databases from the Notion sidebar
    status [ui] — Check active CDP connection to Notion Desktop
    write [ui] — Append text content to the currently open Notion page

  ones
    login [cookie] — ONES Project API — login via Chrome Bridge (POST auth/login); stderr prints export hints for ONES_USER_ID / TOKEN
    logout [cookie] — ONES Project API — invalidate current token (GET auth/logout) via Chrome Bridge
    me [cookie] — ONES Project API — current user (GET users/me) via Chrome Bridge
    my-tasks [cookie] — ONES — my work items (filters/peek + strict must query). Default: assignee=me. Use --mode if your site uses field004 for assignee.
    task [cookie] — ONES — work item detail (GET team/:team/task/:id/info); id is URL segment after …/task/
    tasks [cookie] — ONES Project API — list work items (POST team/:team/filters/peek); use token-info -f json for team uuid
    token-info [cookie] — ONES Project API — session detail (GET auth/token_info) via Chrome Bridge: user, teams, org
    worklog [cookie] — ONES — log work hours on a task (defaults to today; use --date to backfill; endpoint falls back by deployment).

  paperreview
    feedback [public] — Submit feedback for a paperreview.ai review token
    review [public] — Fetch a paperreview.ai review by token
    submit [public] — Submit a PDF to paperreview.ai for review

  pixiv
    detail [cookie] — View illustration details (tags, stats, URLs)
    download [cookie] — Download illustration images from Pixiv
    illusts [cookie] — List a Pixiv artist's illustrations
    ranking [cookie] — Pixiv illustration rankings (daily/weekly/monthly)
    search [cookie] — Search Pixiv illustrations by keyword
    user [cookie] — View Pixiv artist profile

  producthunt
    browse [intercept] — Best products in a Product Hunt category
    hot [intercept] — Today's top Product Hunt launches with vote counts
    posts [public] — Latest Product Hunt launches (optional category filter)
    today [public] — Today's Product Hunt launches (most recent day in feed)

  quark
    ls [cookie] — List files in your Quark Drive
    mkdir [cookie] — Create a folder in your Quark Drive
    mv [cookie] — Move files to a folder in your Quark Drive
    rename [cookie] — Rename a file in your Quark Drive
    rm [cookie] — Delete files from your Quark Drive
    save [cookie] — Save shared files to your Quark Drive
    share-tree [cookie] — Get directory tree from Quark Drive share link as nested JSON

  reddit
    comment [cookie] — Post a comment on a Reddit post
    frontpage [cookie] — Reddit Frontpage / r/all
    hot [cookie] — Reddit 热门帖子
    popular [cookie] — Reddit Popular posts (/r/popular)
    read [cookie] — Read a Reddit post and its comments
    save [cookie] — Save or unsave a Reddit post
    saved [cookie] — Browse your saved Reddit posts
    search [cookie] — Search Reddit Posts
    subreddit [cookie] — Get posts from a specific Subreddit
    subscribe [cookie] — Subscribe or unsubscribe to a subreddit
    upvote [cookie] — Upvote or downvote a Reddit post
    upvoted [cookie] — Browse your upvoted Reddit posts
    user [cookie] — View a Reddit user profile
    user-comments [cookie] — View a Reddit user's comment history
    user-posts [cookie] — View a Reddit user's submitted posts

  reuters
    search [cookie] — Reuters 路透社新闻搜索

  sinablog
    article [cookie] — 获取新浪博客单篇文章详情
    hot [cookie] — 获取新浪博客热门文章/推荐
    search [public] — 搜索新浪博客文章（通过新浪搜索）
    user [cookie] — 获取新浪博客用户的文章列表

  sinafinance
    news [public] — 新浪财经 7x24 小时实时快讯
    rolling-news [cookie] — 新浪财经滚动新闻
    stock [public] — 新浪财经行情（A股/港股/美股）
    stock-rank [cookie] — 新浪财经热搜榜

  smzdm
    search [cookie] — 什么值得买搜索好价

  spotify
    auth [public] — Authenticate with Spotify (OAuth — run once)
    next [public] — Skip to next track
    pause [public] — Pause playback
    play [public] — Resume playback or search and play a track/artist
    prev [public] — Skip to previous track
    queue [public] — Add a track to the playback queue
    repeat [public] — Set repeat mode (off / track / context)
    search [public] — Search for tracks
    shuffle [public] — Toggle shuffle on/off
    status [public] — Show current playback status
    volume [public] — Set playback volume (0-100)

  stackoverflow
    bounties [public] — Active bounties on Stack Overflow
    hot [public] — Hot Stack Overflow questions
    search [public] — Search Stack Overflow questions
    unanswered [public] — Top voted unanswered questions on Stack Overflow

  steam
    top-sellers [public] — Steam top selling games

  substack
    feed [cookie] — Substack 热门文章 Feed
    publication [cookie] — 获取特定 Substack Newsletter 的最新文章
    search [public] — 搜索 Substack 文章和 Newsletter

  taobao
    add-cart [cookie] — 淘宝加入购物车
    cart [cookie] — 查看淘宝购物车
    detail [cookie] — 淘宝商品详情
    reviews [cookie] — 淘宝商品评价
    search [cookie] — 淘宝商品搜索

  tieba
    hot [public] — Tieba hot topics
    posts [cookie] — Browse posts in a tieba forum
    read [cookie] — Read a tieba thread
    search [cookie] — Search posts across tieba

  tiktok
    comment [cookie] — Comment on a TikTok video
    explore [cookie] — Get trending TikTok videos from explore page
    follow [cookie] — Follow a TikTok user
    following [cookie] — List accounts you follow on TikTok
    friends [cookie] — Get TikTok friend suggestions
    like [cookie] — Like a TikTok video
    live [cookie] — Browse live streams on TikTok
    notifications [cookie] — Get TikTok notifications (likes, comments, mentions, followers)
    profile [cookie] — Get TikTok user profile info
    save [cookie] — Add a TikTok video to Favorites
    search [cookie] — Search TikTok videos
    unfollow [cookie] — Unfollow a TikTok user
    unlike [cookie] — Unlike a TikTok video
    unsave [cookie] — Remove a TikTok video from Favorites
    user [cookie] — Get recent videos from a TikTok user

  twitter
    accept [ui] — Auto-accept DM requests containing specific keywords
    article [cookie] — Fetch a Twitter Article (long-form content) and export as Markdown
    block [ui] — Block a Twitter user
    bookmark [ui] — Bookmark a tweet
    bookmarks [cookie] — Fetch Twitter/X bookmarks
    delete [ui] — Delete a specific tweet by URL
    download [cookie] — 下载 Twitter/X 媒体（图片和视频）
    follow [ui] — Follow a Twitter user
    followers [intercept] — Get accounts following a Twitter/X user
    following [intercept] — Get accounts a Twitter/X user is following
    hide-reply [ui] — Hide a reply on your tweet (useful for hiding bot/spam replies)
    like [ui] — Like a specific tweet
    likes [cookie] — Fetch liked tweets of a Twitter user
    notifications [intercept] — Get Twitter/X notifications
    post [ui] — Post a new tweet/thread
    profile [cookie] — Fetch a Twitter user profile (bio, stats, etc.)
    reply [ui] — Reply to a specific tweet, optionally with a local or remote image
    reply-dm [ui] — Send a message to recent DM conversations
    search [intercept] — Search Twitter/X for tweets
    thread [cookie] — Get a tweet thread (original + all replies)
    timeline [cookie] — Fetch Twitter timeline (for-you or following)
    trending [cookie] — Twitter/X trending topics
    unblock [ui] — Unblock a Twitter user
    unbookmark [ui] — Remove a tweet from bookmarks
    unfollow [ui] — Unfollow a Twitter user

  v2ex
    daily [cookie] — V2EX 每日签到并领取铜币
    hot [public] — V2EX 热门话题
    latest [public] — V2EX 最新话题
    me [cookie] — V2EX 获取个人资料 (余额/未读提醒)
    member [public] — V2EX 用户资料
    node [public] — V2EX 节点话题列表
    nodes [public] — V2EX 所有节点列表
    notifications [cookie] — V2EX 获取提醒 (回复/由于)
    replies [public] — V2EX 主题回复列表
    topic [public] — V2EX 主题详情和回复
    user [public] — V2EX 用户发帖列表

  web
    read [cookie] — Fetch any web page and export as Markdown

  weibo
    comments [cookie] — Get comments on a Weibo post
    feed [cookie] — Weibo home timeline (posts from followed users)
    hot [cookie] — 微博热搜
    me [cookie] — My Weibo profile info
    post [cookie] — Get a single Weibo post
    search [cookie] — 搜索微博
    user [cookie] — Get Weibo user profile

  weixin
    download [cookie] — 下载微信公众号文章为 Markdown 格式

  weread
    book [cookie] — View book details on WeRead
    highlights [cookie] — List your highlights (underlines) in a book
    notebooks [cookie] — List books that have highlights or notes
    notes [cookie] — List your notes (thoughts) on a book
    ranking [public] — WeRead book rankings by category
    search [public] — Search books on WeRead
    shelf [cookie] — List books on your WeRead bookshelf

  wikipedia
    random [public] — Get a random Wikipedia article
    search [public] — Search Wikipedia articles
    summary [public] — Get Wikipedia article summary
    trending [public] — Most-read Wikipedia articles (yesterday)

  xianyu
    chat [cookie] — 打开闲鱼聊一聊会话，并可选发送消息
    item [cookie] — 查看闲鱼商品详情
    search [cookie] — 搜索闲鱼商品

  xiaoe
    catalog [cookie] — 小鹅通课程目录（支持普通课程、专栏、大专栏）
    content [cookie] — 提取小鹅通图文页面内容为文本
    courses [cookie] — 列出已购小鹅通课程（含 URL 和店铺名）
    detail [cookie] — 小鹅通课程详情（名称、价格、学员数、店铺）
    play-url [cookie] — 小鹅通视频/音频/直播回放 M3U8 播放地址

  xiaohongshu
    comments [cookie] — 获取小红书笔记评论（支持楼中楼子回复）
    creator-note-detail [cookie] — 小红书单篇笔记详情页数据 (笔记信息 + 核心/互动数据 + 观看来源 + 观众画像 + 趋势数据)
    creator-notes [cookie] — 小红书创作者笔记列表 + 每篇数据 (标题/日期/观看/点赞/收藏/评论)
    creator-notes-summary [cookie] — 小红书最近笔记批量摘要 (列表 + 单篇关键数据汇总)
    creator-profile [cookie] — 小红书创作者账号信息 (粉丝/关注/获赞/成长等级)
    creator-stats [cookie] — 小红书创作者数据总览 (观看/点赞/收藏/评论/分享/涨粉，含每日趋势)
    download [cookie] — 下载小红书笔记中的图片和视频
    feed [intercept] — 小红书首页推荐 Feed (via Pinia Store Action)
    note [cookie] — 获取小红书笔记正文和互动数据
    notifications [intercept] — 小红书通知 (mentions/likes/connections)
    publish [cookie] — 小红书发布图文笔记 (creator center UI automation)
    search [cookie] — 搜索小红书笔记
    user [cookie] — Get public notes from a Xiaohongshu user profile

  xiaoyuzhou
    episode [public] — View details of a Xiaoyuzhou podcast episode
    podcast [public] — View a Xiaoyuzhou podcast profile
    podcast-episodes [public] — List recent episodes of a Xiaoyuzhou podcast (up to 15, SSR limit)

  xueqiu
    comments [cookie] — 获取单只股票的讨论动态
    earnings-date [cookie] — 获取股票预计财报发布日期（公司大事）
    feed [cookie] — 获取雪球首页时间线（关注用户的动态）
    fund-holdings [cookie] — 获取蛋卷基金持仓明细（可用 --account 按子账户过滤）
    fund-snapshot [cookie] — 获取蛋卷基金快照（总资产、子账户、持仓，推荐 -f json 输出）
    groups [cookie] — 获取雪球自选股分组列表（含模拟组合）
    hot [cookie] — 获取雪球热门动态
    hot-stock [cookie] — 获取雪球热门股票榜
    kline [cookie] — 获取雪球股票K线（历史行情）数据
    search [cookie] — 搜索雪球股票（代码或名称）
    stock [cookie] — 获取雪球股票实时行情
    watchlist [cookie] — 获取雪球自选股/模拟组合股票列表

  yahoo-finance
    quote [cookie] — Yahoo Finance 股票行情

  yollomi
    background [cookie] — Generate AI background for a product/object image (5 credits)
    edit [cookie] — Edit images with AI text prompts (Qwen image edit)
    face-swap [cookie] — Swap faces between two photos (3 credits)
    generate [cookie] — Generate images with AI (text-to-image or image-to-image)
    models [public] — List available Yollomi AI models (image, video, tools)
    object-remover [cookie] — Remove unwanted objects from images (3 credits)
    remove-bg [cookie] — Remove image background with AI (free)
    restore [cookie] — Restore old or damaged photos with AI (4 credits)
    try-on [cookie] — Virtual try-on — see how clothes look on a person (3 credits)
    upload [cookie] — Upload an image or video to Yollomi (returns URL for other commands)
    upscale [cookie] — Upscale image resolution with AI (1 credit)
    video [cookie] — Generate videos with AI (text-to-video or image-to-video)

  youtube
    channel [cookie] — Get YouTube channel info and recent videos
    comments [cookie] — Get YouTube video comments
    search [cookie] — Search YouTube videos
    transcript [cookie] — Get YouTube video transcript/subtitles
    video [cookie] — Get YouTube video metadata (title, views, description, etc.)

  yuanbao
    ask [cookie] — Send a prompt to Yuanbao web chat and wait for the assistant response
    new [cookie] — Start a new conversation in Yuanbao web chat

  zhihu
    answer [ui] — Answer a Zhihu question
    comment [ui] — Create a top-level comment on a Zhihu answer or article
    download [cookie] — 导出知乎文章为 Markdown 格式
    favorite [ui] — Favorite a Zhihu answer or article into a specific collection
    follow [ui] — Follow a Zhihu user or question
    hot [cookie] — 知乎热榜
    like [ui] — Like a Zhihu answer or article
    question [cookie] — 知乎问题详情和回答
    search [cookie] — 知乎搜索

  zsxq
    dynamics [cookie] — 获取所有星球的最新动态
    groups [cookie] — 列出当前账号加入的星球
    search [cookie] — 搜索星球内容
    topic [cookie] — 获取单个话题详情和评论
    topics [cookie] — 获取当前星球的话题列表

  external CLIs
    docker [installed] — Docker command-line interface
    dws [auto-install] — DingTalk Workspace CLI — messages, docs, calendar, contacts and more for humans and AI agents
    gh [auto-install] — GitHub CLI — repos, PRs, issues, releases, gists
    lark-cli [auto-install] — Lark/Feishu CLI — messages, documents, spreadsheets, calendar, tasks and 200+ commands for AI agents
    obsidian [installed] — Obsidian vault management — notes, search, tags, tasks, sync
    vercel [auto-install] — Vercel CLI — deploy projects, manage domains, env vars, logs and serverless functions
    wecom-cli [auto-install] — WeCom/企业微信 CLI — contacts, todos, meetings, messages, calendar, docs and smart sheets for AI agents

  532 built-in commands across 84 sites, 7 external CLIs

```
