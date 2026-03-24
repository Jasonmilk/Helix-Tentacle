import httpx
from config import settings
import logging

logger = logging.getLogger("tentacle.prober")

class TentacleProber:
    def __init__(self):
        self.api_url = settings.tuck_url
        self.headers = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {settings.tuck_api_key}" if settings.tuck_api_key else ""
        }

    async def probe_outline(self, text: str, model: str = None) -> str:
        """主入口：执行两次探查以获取完整轮廓"""
        target_model = model or settings.default_probe_model
        logger.info(f"🦑 伸出触手... 开始第一次探查 (Model: {target_model})")
        
        outline1 = await self._first_probe(text, target_model)
        
        if self._is_outline_clear(outline1):
            logger.info("🦑 第一次探查已获得清晰轮廓。")
            return outline1
            
        logger.info("🦑 轮廓存在盲区，触手深入... 开始第二次探查。")
        return await self._second_probe(text, outline1, target_model)

    async def _first_probe(self, text: str, model: str) -> str:
        """第一次探查：截取头尾"""
        sample_size = settings.sample_size
        if len(text) <= sample_size:
            sample = text
        else:
            head = text[:sample_size // 2]
            tail = text[-sample_size // 2:]
            sample = f"{head}\n\n...[触手略过了中间 {len(text) - sample_size} 个字符]...\n\n{tail}"
            
        prompt = (
            "你是一个高度敏锐的文档探针。请阅读以下长文档的【头尾片段】，"
            "提取出文档的【核心大纲】和【主要章节轮廓】。\n"
            "要求：只关注骨架，忽略细节。如果发现明显的断层，请在输出中注明“中间部分缺失”。\n\n"
            f"文档片段:\n{sample}"
        )
        return await self._call_tuck(prompt, model)

    async def _second_probe(self, text: str, first_outline: str, model: str) -> str:
        """第二次探查：均匀采样中间部分"""
        mid_start = len(text) // 3
        mid_sample = text[mid_start : mid_start + settings.sample_size]
        
        prompt = (
            f"这是文档的初步轮廓：\n{first_outline}\n\n"
            f"这是文档中间部分的节选：\n{mid_sample}\n\n"
            "请结合这段中间节选，补充并完善初步轮廓，输出最终的完整、连贯的大纲。"
        )
        return await self._call_tuck(prompt, model)

    def _is_outline_clear(self, outline: str) -> bool:
        """启发式判断轮廓是否清晰"""
        # 如果模型明确抱怨缺失，或者轮廓太短，则认为不清晰
        if "中间部分缺失" in outline or "断层" in outline or len(outline) < 100:
            return False
        return True

    async def _call_tuck(self, prompt: str, model: str) -> str:
        payload = {
            "model": model,
            "messages":[{"role": "user", "content": prompt}],
            "temperature": 0.2 # 探查需要客观准确
        }
        async with httpx.AsyncClient(timeout=120.0) as client:
            try:
                resp = await client.post(self.api_url, json=payload, headers=self.headers)
                resp.raise_for_status()
                return resp.json()["choices"][0]["message"]["content"]
            except Exception as e:
                logger.error(f"触手调用 Tuck 失败: {e}")
                return f"[Tentacle Error] 探查失败: {str(e)}"
