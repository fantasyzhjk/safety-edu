use chrono::{TimeZone, Utc};
use docopt::Docopt;
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use json::object;
use json::JsonValue;
use paris::*;
use rand::Rng;
use reqwest::*;
use std::fs;
use std::process;
use std::thread;
use std::time::Duration;

const USAGE: &'static str = "

Usage:
  safety_edu [--hours=<t>] study <username> <password> [<school>]
  safety_edu [--score=<s>] exam <username> <password> [<school>]
  safety_edu info <username> <password> [<school>]
  safety_edu search <school>
  safety_edu (-h | --help)
  safety_edu (-v | --version)

Options:
  -h --help         Show this screen.
  -v --version      Show version.
  --hours=<t>       Study time [default: 6].
  --score=<s>       Final score [default: 100].
";

fn post(src: &str, params: &[(&str, &str)]) -> reqwest::blocking::RequestBuilder {
    let client = reqwest::blocking::Client::new();
    let resp = client.post(src)
    .query(params)
    .header("Content-Length", 0)
    .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/80.0.3987.149 Safari/537.36");
    resp
}

struct SafetyEdu {
    cookies: String,
}

impl SafetyEdu {
    fn new(school_id: &str, user_name: &str, user_pwd: &str) -> SafetyEdu {
        SafetyEdu::login(school_id, user_name, user_pwd)
    }

    fn login(school_id: &str, user_name: &str, user_pwd: &str) -> SafetyEdu {
        let resp = post(
            "https://aq.fhmooc.com/api/common/Login/login",
            &[
                ("schoolId", school_id),
                ("userName", user_name),
                ("userPwd", user_pwd),
            ],
        )
        .send()
        .unwrap();
        if resp.status() != 200 {
            log!("登陆失败，状态码: {}", resp.status());
            process::exit(1);
        }
        let cookies = resp
            .headers()
            .get_all("Set-Cookie")
            .into_iter()
            .map(|v| v.to_str().unwrap().split(";").collect::<Vec<_>>()[0])
            .collect::<Vec<_>>()
            .join(";");
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        if j["code"] != 1 {
            log!("登陆失败，错误消息: {}", j["msg"]);
            process::exit(1);
        }
        SafetyEdu { cookies: cookies }
    }

    fn get_school_list() -> Result<JsonValue> {
        let resp = post("https://aq.fhmooc.com/api/common/Login/getSchoolList", &[]).send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error")["list"].clone();
        Ok(j)
    }

    fn get_auth_info(&self) -> Result<JsonValue> {
        let resp = post("https://aq.fhmooc.com/api/common/Login/getAuthInfo", &[])
            .header("Cookie", self.cookies.clone())
            .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn get_module_info(&self, module_id: &str) -> Result<JsonValue> {
        let resp = post(
            "https://aq.fhmooc.com/api/portal/CourseIndex/getModuleInfo",
            &[("moduleId", module_id)],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn add_my_mooc_module(&self, module_id: &str) -> Result<JsonValue> {
        let resp = post(
            "https://aq.fhmooc.com/api/design/LearnCourse/addMyMoocModule",
            &[("moduleId", module_id)],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn save_stu_ques_answer(
        &self,
        answer: &str,
        ques_id: &str,
        paper_stu_id: &str,
        paper_id: &str,
    ) -> Result<JsonValue> {
        // ques_id  1: 单选  2: 多选  3: 判断
        let answer_json = json::stringify(object! {
            quesId: ques_id,
            answer: answer
        });
        let resp = post(
            "https://aq.fhmooc.com/api/design/PaperStudent/saveStuQuesAnswer",
            &[
                ("paperStuId", paper_stu_id),
                ("paperId", paper_id),
                ("quesId", ques_id),
                ("answerJson", answer_json.as_str()),
            ],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn add_stu_time(
        &self,
        module_id: &str,
        cource_id: &str,
        cell_id: &str,
        hours: i32,
    ) -> Result<JsonValue> {
        let t = rand::thread_rng().gen_range(hours * 18..hours * 18 + 30);
        let resp = post(
            "https://aq.fhmooc.com/api/design/LearnCourse/statStuProcessCellLogAndTimeLong",
            &[
                ("moduleIds", module_id),
                ("courseId", cource_id),
                ("cellId", cell_id),
                ("auvideoLength", t.to_string().as_str()),
                ("videoTimeTotalLong", t.to_string().as_str()),
            ],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn get_module_list(&self) -> Result<JsonValue> {
        let resp = post(
            "https://aq.fhmooc.com/api/portal/CellManager/getModuleList",
            &[],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error")["list"].clone();
        Ok(j)
    }

    fn get_my_study_timer_summary(&self) -> Result<JsonValue> {
        let resp = post(
            "https://aq.fhmooc.com/api/design/LearnCourse/getMyStudyTimerSummary",
            &[],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn get_stu_paper(&self, course_id: &str) -> Result<JsonValue> {
        let resp = post(
            "https://aq.fhmooc.com/api/design/PaperStudent/getStuPaper",
            &[("courseId", course_id)],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap())
            .expect("json parse error")
            .clone();
        Ok(j)
    }

    fn get_couse_paper_info(&self, course_id: &str) -> Result<JsonValue> {
        let resp = post(
            "https://aq.fhmooc.com/api/design/LearnPaper/getCousePpaerInfo",
            &[("courseId", course_id)],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        let j = json::parse(&resp.text().unwrap()).expect("json parse error");
        Ok(j)
    }

    fn sumit_stu_paper(&self, paper_stu_id: &str, paper_id: &str) -> Result<()> {
        let resp = post(
            "https://aq.fhmooc.com/api/design/PaperStudent/submitStuPaper",
            &[("paperStuId", paper_stu_id), ("paperId", paper_id)],
        )
        .header("Cookie", self.cookies.clone())
        .send()?;
        if resp.status() != 200 {
            panic!("failed: {}", resp.status());
        }
        Ok(())
    }
}

fn exam(safety_edu: &SafetyEdu, score: i32) -> Result<()> {
    let course_id = "qkcfawcsxyrom0zrwghhwq"; // 综合测评
    let paper = safety_edu.get_stu_paper(course_id.clone())?;
    let paper_name = paper["paperName"].as_str().unwrap();
    let paper_id = paper["paperId"].as_str().unwrap();
    let paper_stu_id = paper["paperStuId"].as_str().unwrap();
    let questions = paper["stuPaperQuesList"].members();
    let answer_list = json::parse(fs::read_to_string("./answers.json").unwrap().as_str()).unwrap();
    info!(" 成功获取试卷: {}, 试卷id: {}", paper_name, paper_id);

    let mut logger = Logger::new();
    let mut cs = 0;
    for q in questions {
        cs += 1;
        if cs > score {
            logger.loading(format!(" ({}/100) 达到分数上线，跳过该题", cs));
            continue;
        }
        logger.done();
        let ques_id = q["quesId"].as_str().unwrap();
        let a = match answer_list.members().find(|v| v["id"] == ques_id) {
            Some(x) => x,
            None => {
                error!("无此题目");
                continue;
            }
        }
        .clone();
        let answer = a["answer"]
            .members()
            .map(|v| v.as_str().unwrap())
            .collect::<Vec<_>>()
            .join("；");
        let timu = a["content"]
            .to_string()
            .chars()
            .into_iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>();

        logger.loading(format!(
            " ({}/100) [{}] {} 答案: {}",
            cs,
            a["type"],
            timu[0..if timu.len() >= 32 { 32 } else { timu.len() }]
                .concat()
                .as_str(),
            answer
        ));
        safety_edu.save_stu_ques_answer(answer.as_str(), ques_id, paper_stu_id, paper_id)?;
        thread::sleep(Duration::from_secs(rand::thread_rng().gen_range(5..10)));
    }
    safety_edu.sumit_stu_paper(paper_stu_id, paper_id)?;
    thread::sleep(Duration::from_secs(3));
    let paper_info =
        safety_edu.get_couse_paper_info(course_id.clone())?["paperStudentList"][0].clone();
    logger.done().success(format!(
        " 考试结束，共用时: <b>{}</>, 总分: <b>{}</>",
        paper_info["answerTimeStr"], paper_info["studentTotalScore"]
    ));
    Ok(())
}

fn study(safety_edu: &SafetyEdu, hours: i32) -> Result<()> {
    info!(" 开始学习, 预期学习时长 <b>{}</> 小时", hours);
    let start_time = Utc::now();
    let module_list = safety_edu.get_module_list()?;
    let module_id = module_list[0]["id"].as_str().unwrap();
    safety_edu.add_my_mooc_module(module_id.clone())?;
    let module_info = safety_edu.get_module_info(module_id.clone())?;
    let cell_ids = module_info["cellList"]
        .members()
        .filter(|v| v["docId"] != "")
        .map(|v| v["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let cource_id = module_info["moduleInfo"]["courseOpenId"].as_str().unwrap();

    // 学习课程
    let pb = ProgressBar::new(cell_ids.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos:>5}/{len:5} ({eta})")
        .progress_chars("#>-"));

    for cell_id in cell_ids.iter().progress_with(pb.clone()) {
        safety_edu.add_stu_time(module_id.clone(), cource_id, cell_id, hours)?;
    }

    success!(
        " 学习结束，共用时 <b>{:.2}</> 秒",
        (Utc::now() - start_time).num_seconds()
    );
    let study_timer_summary = safety_edu.get_my_study_timer_summary()?;
    log!(
        "   累计学习时长: <b>{}</>\n   累计学习次数: <b>{}次</>\n   累计学习总课程: <b>{}门</>",
        Utc.timestamp(
            study_timer_summary["cumulativeStudyTimer"]
                .as_i64()
                .unwrap(),
            0
        )
        .format("%-Hh %-Mm %-Ss"),
        study_timer_summary["cumulativeStudyCount"],
        study_timer_summary["cumulativeStudyCourse"]
    );
    Ok(())
}

fn search_school(school_name: &str) -> Result<()> {
    let school_list = SafetyEdu::get_school_list()?;
    let school_info = school_list
        .members()
        .filter(|v| v["name"].as_str().unwrap().contains(school_name));
    info!("<b>搜索到的学校 ↓</>");
    for school in school_info.clone() {
        log!("  <b>ID:</> {:<25}<b>学校:</> {}", school["id"], school["name"]);
    }
    info!("<b>共 {} 所 ↑</>", school_info.count());
    Ok(())
}

fn print_study_info(safety_edu: &SafetyEdu) -> Result<()> {
    let study_timer_summary = safety_edu.get_my_study_timer_summary()?;
    log!(
        "   累计学习时长: <b>{}</>\n   累计学习次数: <b>{}次</>\n   累计学习总课程: <b>{}门</>",
        Utc.timestamp(
            study_timer_summary["cumulativeStudyTimer"]
                .as_i64()
                .unwrap(),
            0
        )
        .format("%-Hh %-Mm %-Ss"),
        study_timer_summary["cumulativeStudyCount"],
        study_timer_summary["cumulativeStudyCourse"]
    );
    let course_id = "qkcfawcsxyrom0zrwghhwq"; // 综合测评
    let paper_info =
        safety_edu.get_couse_paper_info(course_id.clone())?["paperStudentList"][0].clone();
    log!(
        "   上次考试，共用时: <b>{}</>, 总分: <b>{}</>",
        paper_info["answerTimeStr"],
        paper_info["studentTotalScore"]
    );
    Ok(())
}

fn main() -> Result<()> {
    let args = Docopt::new(USAGE)
        .and_then(|dopt| dopt.parse())
        .unwrap_or_else(|e| e.exit());
    if args.get_bool("--version") {
        log!("safety_edu v0.0.1");
        process::exit(0);
    }
    let mut school_key = args.get_str("<school>");
    if school_key == "" {
        school_key = "i7kbawgs4kfesy5it2xp0w"
    };
    let hours = args.get_str("--hours").parse().unwrap();
    let score = args.get_str("--score").parse().unwrap();
    let user_name = args.get_str("<username>");
    let user_pwd = args.get_str("<password>");

    if args.get_bool("search") {
        search_school(school_key)?;
        process::exit(0);
    }

    let school_info = match SafetyEdu::get_school_list()?
        .members()
        .find(|v| v["id"] == school_key)
    {
        Some(x) => x,
        None => {
            log!("无此学校");
            process::exit(1);
        }
    }
    .clone();
    // log!("{}", school_info["id"]);
    let safety_edu = SafetyEdu::new(school_info["id"].as_str().unwrap(), user_name, user_pwd);
    // log!("{}", safety_edu.cookies);
    warn!(
        " 已登录，用户: <yellow><b>{}</>",
        safety_edu.get_auth_info()?["displayName"]
    );

    if args.get_bool("info") {
        print_study_info(&safety_edu)?;
        process::exit(0);
    }
    if args.get_bool("study") {
        study(&safety_edu, hours)?; // 开始学习
        process::exit(0);
    }
    if args.get_bool("exam") {
        exam(&safety_edu, score)?;
        process::exit(0);
    }
    Ok(())
}
