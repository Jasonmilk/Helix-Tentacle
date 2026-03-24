import httpx
from config import settings
import logging

logger = logging.getLogger("tentacle.dehydrator")

class TentacleDehydrator:
    def __init__(self):
        self.api_url = settings.tuck_url
        self.headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {settings.tuck_api_key}" if settings.tuck_api_key else ""
        }

    async def dehydrate(self, text: str, outline: str, purpose: str, model: str = None) -> str:
        """基于大纲和目的，对长文档进行降维脱水"""
        target_model = model or settings.default_probe_model
        logger.info(f"💧 开始定向脱水 (目的: {purpose})...")
        
        # 为了防止爆显存，脱水时也限制输入长度（根据实际情况可做成分块 Map-Reduce，这里先做截断演示）
        safe_text = text[:settings.sample_size * 3] 
        if len(text) > settings.sample_size * 3:
            safe_text += "\n...[超长截断]..."

        prompt = (
            "你是一个无情的文档脱水器。你的首要任务是满足用户的【脱水目的】。\n\n"
            f"【脱水目的】：{purpose}\n"
            f"【文档全局大纲】：\n{outline}\n\n"
            "请根据大纲的指引，从以下文档内容中提取出符合【脱水目的】的极简核心信息，剔除所有冗余废话。\n"
            "要求：输出结构清晰（可使用Markdown），字数尽量精简。\n\n"
            f"【文档内容】：\n{safe_text}"
        )
        
        payload = {
            "model": target_model,
            "messages": [{"role": "user", "content": prompt}],
            "temperature": 0.3
        }
        
        async with httpx.AsyncClient(timeout=180.0) as client:
            try:
                resp = await client.post(self.api_url, json=payload, headers=self.headers)
                resp.raise_for_status()
                return resp.json()["choices"][0]["message"]["content"]
            except Exception as e:
                logger.error(f"触手脱水调用 Tuck 失败: {e}")
                return f"[Tentacle Error] 脱水失败: {str(e)}"
