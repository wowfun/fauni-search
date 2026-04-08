# TATDQA Page Image Fixtures

该目录由 `scripts/extract_tatdqa_page_images.py` 生成。

- 数据集来源：<https://huggingface.co/datasets/vidore/tatdqa_train>
- 源 parquet 目录：`data/datasets/tatdqa_train/data`
- 导出模式：`sample`
- 导出图片数：`32` / 唯一页图总数 `2481`
- 采样大小：`32`
- `manifest.json` 保存每张图片的来源页、代表性问答、尺寸与哈希。
- `images/` 中的 PNG 文件按稳定顺序命名，便于后续自动化测试直接引用。
