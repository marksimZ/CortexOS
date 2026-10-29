"""Reproduce/check the reviewed v2 proposal. Never overwrite an existing dataset.

Only trusted, fixed authoring calculations run here; no submitted Python is loaded.
Usage: py -3 -X utf8 scripts/benchmark_core60_v2.py [--write-new]
"""
import argparse
import copy
import hashlib
import itertools
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / 'apps/desktop/src-tauri/datasets/cortex-core-60'
DEST = BASE.parent / 'cortex-core-60-v2'
DIALECT = 'https://json-schema.org/draft/2020-12/schema'
AS_OF = '2026-07-02T12:00:00+08:00'
WHITESPACE = list(range(9, 14)) + list(range(28, 33)) + [133, 160, 5760] + list(range(8192, 8203)) + [8232, 8233, 8239, 8287, 12288]


def text(value):
    return json.dumps(value, ensure_ascii=False, indent=2) + '\n'


def digest(value):
    return hashlib.sha256(value).hexdigest()


def case_hash(case):
    raw = {k: v for k, v in case.items() if k != 'content_hash'}
    return digest(json.dumps(raw, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode())


def obj(properties, required=None):
    return {'type': 'object', 'properties': properties, 'required': list(properties) if required is None else required, 'additionalProperties': False}


INTEGER = {'type': 'integer'}
ARG_INT = {'type': 'integer', 'minimum': -1000000000, 'maximum': 1000000000}
INT_LIST = {'type': 'array', 'items': ARG_INT, 'maxItems': 64}
STRING = {'type': 'string', 'maxLength': 64}

# Fixed authored inputs; original examples remain the prefix of each list.
EXTRAS = {
    'code-001': [[[7]], [[-5, -2]], [[2, 2, 2]], [[1000000000] * 64]],
    'code-002': [[[2, 2, 2]], [[2]], [[-3]], [[-2, 0, 4]], [[0] * 64]],
    'code-003': [['a'], [' a\t'], ['aa'], ['😀a'], ['e\u0301'], ['👩\u200d💻'], ['a' * 64]],
    'code-004': [[[-2, -2, 1, -2]], [[4]], [[3, 2, 1]], [[1] * 64]],
    'code-005': [['Aa'], ['abba'], ['x'], [' a'], ['a,a'], ['😀a😀'], ['a' * 64]],
    'code-006': [[0, 0, 10], [10, 0, 10], [-3, -5, -1], [3, 3, 3], [-1000000000, -5, 5], [1000000000, -5, 5]],
    'code-007': [[n] for n in range(13) if n not in [0, 1, 5, 12]],
    'code-008': [[' \t\n'], ['你好世界'], ['word'], ['a\u3000b'], ['a\u00a0b'], [''.join(map(chr, WHITESPACE))], ['\u200b'], ['a' * 64]],
    'code-009': [[[-1, 0], []], [[-2, -2], [-2]], [[1], [2, 3, 4, 5]], [[1, 1], [1, 1]], [[0] * 64, [0] * 64]],
    'code-010': [[[0]], [[-4]], [[-1, -2, -3]], [[0, 0, 0]], [[2, 2, 2]], [[1000000000] * 64]],
}


def control(case_id, args):
    # Independent stdlib calculations, not execution of a candidate's named function.
    return {
        'code-001': lambda: sum(args[0]),
        'code-002': lambda: sum(v % 2 == 0 for v in args[0]),
        'code-003': lambda: ''.join(reversed(args[0])),
        'code-004': lambda: list(dict.fromkeys(args[0])),
        'code-005': lambda: list(args[0]) == list(reversed(args[0])),
        'code-006': lambda: min(args[2], max(args[1], args[0])),
        'code-007': lambda: math.factorial(args[0]),
        'code-008': lambda: len(args[0].split()),
        'code-009': lambda: sorted(itertools.chain(args[0], args[1])),
        'code-010': lambda: list(itertools.accumulate(args[0])),
    }[case_id]()


def changed_fields(old, new, prefix=''):
    result = []
    for key in sorted(set(old) | set(new)):
        if key == 'content_hash':
            continue
        path = f'{prefix}.{key}' if prefix else key
        a, b = old.get(key), new.get(key)
        if a == b:
            continue
        if isinstance(a, dict) and isinstance(b, dict):
            result.extend(changed_fields(a, b, path))
        else:
            result.append({'field': path, 'old': a, 'new': b})
    return result


def build():
    assert digest((BASE / 'manifest.json').read_bytes()) == '3595faf98b35865e00ba7f156ba89a21f6f5406f8f1b2908f41df031c3f882cc'
    assert digest((BASE / 'cases.json').read_bytes()) == '784089a5243bb3dc1cf4b723b351df19c5006ff983d97c3f4e8f9a946bc88806'
    original = json.loads((BASE / 'cases.json').read_text(encoding='utf-8'))
    cases = copy.deepcopy(original)
    for c in cases:
        c['dataset_version'] = 2
        c['scoring']['version'] = 2
        c['scoring']['contract'] = {}
        cfg = c['scoring']['contract']
        cid = c['case_id']
        if cid.startswith('zh-'):
            cfg.update(normalization='trim_unicode_whitespace', case_sensitive=True, unicode_folding=False)
            if cid == 'zh-001': c['input']['prompt'] += '使用小写ASCII字母。'
            if cid == 'zh-002': c['input']['prompt'] = '按教育部《重编国语辞典修订本》的部首分类，“明”的部首是哪一部？只写一个汉字。'
            if cid == 'zh-003': c['expected']['answers'].append('論語')
            if cid == 'zh-005': c['input']['prompt'] = c['input']['prompt'].replace('一年四季中', '按通常春夏秋冬四季顺序')
            if cid == 'zh-006': c['input']['prompt'] += '使用半角ASCII数字，不加前导零。'
            if cid == 'zh-008': c['input']['prompt'] = '地球长期稳定的天然卫星叫什么？只写中文名称。'
            if cid == 'zh-011': c['expected']['answers'].append('公尺')
            if cid == 'zh-012': c['expected']['answers'].append('秒钟')
            if cid == 'zh-014':
                c['expected']['answers'].append('增添')
                c['input']['prompt'] = '从“增加、增多、增添”中选一个“减少”的反义词，只返回所选双字词。'
            if cid != 'zh-003': c['input']['prompt'] += '中文使用简体字（无简繁差异的字照常书写）。'
        elif cid.startswith('math-'):
            cfg.update(normalization='trim_unicode_whitespace', syntax='ascii_minus_optional_digits', integer_range='i64')
            if cid == 'math-006': c['input']['prompt'] = c['input']['prompt'].replace('数列2、5、8、11', '等差数列2、5、8、11')
            c['input']['prompt'] += '格式为可选ASCII负号及半角十进制数字；不用加号、小数点或中文数字。'
        elif cid.startswith('code-'):
            e = c['expected']
            for t in e['tests']:
                expected = control(cid, t['args'])
                assert type(expected) is type(t['output']) and expected == t['output']
            e['tests'].extend({'args': args, 'output': control(cid, args)} for args in EXTRAS[cid])
            params = [STRING] if cid in ['code-003', 'code-005', 'code-008'] else [INT_LIST]
            if cid == 'code-006': params = [ARG_INT] * 3
            if cid == 'code-007': params = [{'type': 'integer', 'minimum': 0, 'maximum': 12}]
            if cid == 'code-009': params = [INT_LIST, INT_LIST]
            result = INTEGER
            if cid == 'code-003': result = {'type': 'string'}
            if cid == 'code-005': result = {'type': 'boolean'}
            if cid in ['code-004', 'code-009', 'code-010']: result = {'type': 'array', 'items': INTEGER}
            cfg.update(python_version='3.13', parameter_schemas=params, return_schema=result, strict_python_types=True, input_mutation='forbidden', preconditions=[])
            if cid == 'code-006': cfg['preconditions'] = ['low<=high']
            if cid == 'code-009': cfg['preconditions'] = ['a_and_b_sorted_ascending']
            if cid == 'code-003':
                cfg['text_units'] = 'unicode_codepoints'
                c['input']['prompt'] = c['input']['prompt'].replace('按Unicode字符逆序返回', '按Unicode码点逆序返回，不做规范化，不按字素簇合并；组合符和ZWJ也独立逆序')
            if cid == 'code-008':
                cfg['whitespace_codepoints'] = WHITESPACE
                c['input']['prompt'] = c['input']['prompt'].replace('按任意连续空白分词', '按Python 3.13 str.split()无参数的连续空白规则分词（包括U+3000和U+00A0；U+200B不算空白）')
            c['input']['prompt'] = c['input']['prompt'].replace('Python 3实现', 'Python 3.13实现')
            c['input']['prompt'] += ' 不得修改任何输入参数；整数返回必须为int（不接受bool或float），布尔返回必须为bool，列表返回必须为list且整数元素为int；不允许用tuple代替list。整数输入绝对值不超过10^9，每个输入列表或字符串长度不超过64；阶乘仍只接收0到12。只测试题目定义域内输入。'
        elif cid.startswith('rag-'):
            old = c['expected']
            i = int(cid[-3:])
            facts = [{ 'opening_time': '09:00'}, {'owner': '林澄'}, {'retention_days': 30}, {}, {}, {'budget_a_wan_yuan': 80, 'budget_b_wan_yuan': 95}, {'quota_a': 2, 'quota_b': 3}, {'floor': 2}, {'delivery_date': '2026-06-18'}, {'rest_weekday': '周二'}][i-1]
            fields = {key: {'type': 'integer' if type(value) is int else 'string'} for key, value in facts.items()}
            if i == 4: fields = {'saturday_price_yuan': INTEGER}
            if i == 5: fields = {'start_year': INTEGER}
            if i == 9: fields['delivery_date']['format'] = 'date'
            citations = []
            evidence = {}
            for key, span in zip(facts, old['citations']):
                citations.append({'claim_id': key, **span})
                count = len(next(s['lines'] for s in c['fixture'] if s['id'] == span['source_id']))
                evidence[key] = [{'source_id': span['source_id'], 'start': start, 'end': end} for start in range(1, span['start']+1) for end in range(span['end'], count+1)]
            reason = 'insufficient_evidence' if old['disposition'] == 'unanswerable' else 'conflicting_evidence' if old['disposition'] == 'conflict' else None
            c['expected'] = {'kind': 'json', 'answer': {'status': old['disposition'], 'facts': facts, 'reason': reason, 'citations': citations}}
            aliases = {'opening_time': ['09:00', '9:00', '9点', '上午九点']} if i == 1 else {'rest_weekday': ['周二', '星期二']} if i == 10 else {}
            cfg.update(as_of=AS_OF, fact_aliases=aliases, evidence=evidence, citation_policy='enumerated_supporting_spans', duplicate_citations='reject', extra_facts='reject', answer_and_citations='score_separately', full_pass_requires='all_dimensions', format_assertion=True, source_policy={}, schema_use='grading_only')
            if i == 10:
                cfg['source_policy'] = {'old': {'effective_from': '2026-01-01T00:00:00+08:00'}, 'new': {'effective_from': '2026-07-01T00:00:00+08:00', 'supersedes': 'old'}}
                c['input']['prompt'] = c['input']['prompt'].replace('按照最新生效通知', f'以{AS_OF}为场景时刻，按照当时最新生效通知')
            question = c['input']['prompt'].split('只依据所附资料')[0]
            c['input']['prompt'] = question + '只依据所附资料，返回单个JSON对象，字段为status、facts、reason、citations，不加围栏或额外字段。facts使用以下事实字段：' + json.dumps(fields, ensure_ascii=False) + '。只在可答时填写事实值，不写解释句；资料不足时status=unanswerable、facts={}、reason=insufficient_evidence、citations=[]；同条件证据冲突时status=conflict，填写双方事实，reason=conflicting_evidence，引用双方；可答时status=answerable、reason=null。引用格式为{"claim_id":"对应事实字段","source_id":"资料ID","start":1,"end":1}，行号从1开始且首尾包含。每条引用须完整支持该事实，可包含相邻无冲突行；不得引用无关行冒充支持。场景时刻固定为' + AS_OF + '，不使用机器当前时间。同条件冲突材料无额外权威优先级。'
            citation_schema = obj({'claim_id': {'type': 'string'}, 'source_id': {'type': 'string'}, 'start': {'type': 'integer', 'minimum': 1}, 'end': {'type': 'integer', 'minimum': 1}})
            c['input']['response_schema'] = {'$schema': DIALECT, **obj({'status': {'type': 'string', 'enum': ['answerable', 'unanswerable', 'conflict']}, 'facts': obj(fields, []), 'reason': {'type': ['string', 'null'], 'enum': [None, 'insufficient_evidence', 'conflicting_evidence']}, 'citations': {'type': 'array', 'items': citation_schema}})}
        elif cid.startswith('schema-'):
            cfg.update(dialect=DIALECT, duplicate_keys='reject', comparison='typed_json', array_order='significant', format_assertion=True)
            c['input']['response_schema']['$schema'] = DIALECT
            if cid == 'schema-004': c['input']['response_schema']['properties']['date']['format'] = 'date'
            c['input']['prompt'] += '对象键顺序不限，数组顺序必须符合题意；禁止重复键和额外字段。整数允许数学等价的整数表示（如28与28.0），不得用布尔或字符串代替数值；所有声明字段必填，null不能省略。日期须为真实日历日期的YYYY-MM-DD格式。'
        elif cid.startswith('tool-'):
            cfg.update(dialect=DIALECT, duplicate_keys='reject', comparison='typed_json', call_count=1, native_only=True, execute=False)
            if cid == 'tool-001': c['input']['prompt'] = c['input']['prompt'].replace('当天', f'（场景日期2026-07-02）的')
            if cid == 'tool-002': c['input']['prompt'] = c['input']['prompt'].replace('最多3条', '将limit明确设为3（请求上限3条）')
            c['input']['prompt'] += '必须恰好产生一个原生调用；参数名称和值按题意，所有声明参数必填且禁止额外参数。参数对象键顺序不限，整数参数接受数学等价的整数表示。这里只验证调用对象，不执行工具，也不要求查询结果。'
        c['content_hash'] = case_hash(c)

    schema = json.loads((BASE / 'case.schema.json').read_text(encoding='utf-8'))
    schema['$id'] = 'urn:cortexos:benchmark:case:2'
    schema['title'] = 'Cortex Core 60 Smoke case v2'
    schema['properties']['dataset_version']['const'] = 2
    schema['properties']['expected']['oneOf'] = [s for s in schema['properties']['expected']['oneOf'] if s['properties']['kind']['const'] != 'evidence']
    scoring = schema['properties']['scoring']
    scoring['properties']['version']['const'] = 2
    scoring['properties']['contract'] = {'type': 'object', 'minProperties': 1}
    scoring['required'].append('contract')
    rules = {'version': 2, 'document_type': 'scoring_specification', 'review_status': 'needs_human_review', 'runtime_status': 'not_implemented_in_stage_a', 'result_states': ['Passed', 'Failed', 'Unsupported', 'Not Run', 'Execution Error', 'Scoring Error'], 'denominators': 'Only valid scored items enter correctness; show planned coverage, execution success and each excluded state separately.', 'rules': []}
    policies = {
        'exact_alias': 'Trim only leading/trailing Unicode whitespace. Case-sensitive literal membership in per-case aliases. No Unicode/case folding.',
        'integer_equal': 'Trim Unicode whitespace; optional ASCII minus followed by ASCII decimal digits; i64 numeric comparison. No plus, decimals, expressions or implicit units.',
        'python_assertions': 'Docker Linux only, pinned Python 3.13 image digest before execution. Exact Python return types before serialization; deep input mutation check; per-assertion result. No network/socket/sensitive mounts or host fallback. Freeze CPU/RAM/PID/time/output limits; clean owned container and files. Sandbox unavailable is Execution Error, never Failed.',
        'evidence_match': 'Strict structured status/facts/reason/citations. Compare typed facts with explicit aliases; adjudicate status/refusal/conflict/as_of separately. Citation is supporting only when its claim_id and full span are in the annotated evidence map; reject duplicate/unknown citations. Answer and citation correctness separate; all dimensions required for full pass. Zero citations precision=N/A; missing required evidence cannot pass.',
        'schema_semantic': 'Strict single JSON, reject duplicate keys/nonfinite values. JSON Schema 2020-12 with date format assertion; semantic comparison of typed values. Object order irrelevant, arrays ordered, integer mathematical equivalence, bool distinct from numbers, null distinct from missing. Unknown scorer vocabulary is Scoring Error.',
        'native_tool_equal': 'Exactly one actual structured Provider tool_call, never text JSON. Validate name/required arguments/types/no extras and semantic values with integer equivalence. No side effects or tool dispatch. Unsupported only with capability evidence; missing capture is system Execution Error, not model failure.',
    }
    for rule in json.loads((BASE / 'scoring-rules.json').read_text(encoding='utf-8'))['rules']:
        rules['rules'].append({**rule, 'version': 2, 'normalization': policies[rule['id']], 'contract_source': 'case.scoring.contract'})
    notes = {'from_version': 1, 'to_version': 2, 'review_status': 'needs_human_review', 'author': 'AI-assisted technical revision; no independent human sign-off', 'entries': []}
    for old, new in zip(original, cases):
        prefix = new['case_id'].split('-')[0]
        issue = {'zh': ['Q-01', 'Q-07'], 'math': ['Q-01', 'Q-02'], 'code': ['Q-08', 'Q-09'], 'rag': ['Q-03', 'Q-04', 'Q-10'], 'schema': ['Q-11'], 'tool': ['Q-02', 'Q-05']}[prefix]
        notes['entries'].append({'case_id': old['case_id'], 'old_hash': old['content_hash'], 'new_hash': new['content_hash'], 'audit_topics': issue, 'reason': {'zh': 'Make format/aliases explicit without global folding.', 'math': 'Declare integer syntax; math-006 explicitly arithmetic progression.', 'code': 'Preserve original assertions, add domain-valid boundaries and strict type/input contract.', 'rag': 'Replace prose matching with typed facts, enumerated supporting spans, explicit refusal/conflict and fixed time.', 'schema': 'Freeze dialect/types/required fields/date and structural/semantic comparison.', 'tool': 'Align explicit requested arguments with native single-call contract; no dispatch.'}[prefix], 'changes': changed_fields(old, new)})
    files = {'cases.json': text(cases), 'case.schema.json': text(schema), 'scoring-rules.json': text(rules), 'revision-notes.json': text(notes)}
    manifest = json.loads((BASE / 'manifest.json').read_text(encoding='utf-8'))
    manifest.update(version=2, schema_version=2, name='Cortex Core 60 v2 · Smoke Test · 待验收', level='smoke', runtime_status='not_enabled_in_stage_a')
    manifest['files'] = {name: digest(data.encode()) for name, data in files.items()}
    manifest['provenance'].update(authoring='Versioned technical revision against Q-01..Q-12; independent human sign-off pending', limitations='Smoke only; no formal capability ranking. Stage A validates authored data only; v2 runtime scorers pending Stage B. Original v1 immutable.')
    manifest['prompt_template']['version'] = 2
    manifest['prompt_template']['rendering'] = 'system + input.prompt + numbered fixtures; schema-* response_schema is Provider format, rag-* response_schema is grading-only (completion capability); tool-* uses actual tools request field. expected/scoring/revision notes are never sent.'
    files['manifest.json'] = text(manifest)
    return files


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write-new', action='store_true')
    args = parser.parse_args()
    files = build()
    if args.write_new:
        DEST.mkdir(exist_ok=False)
        for name, data in files.items():
            with (DEST / name).open('x', encoding='utf-8', newline='\n') as stream:
                stream.write(data)
    else:
        for name, data in files.items():
            assert (DEST / name).read_bytes() == data.encode(), f'Non-reproducible asset: {name}'
    cases = json.loads(files['cases.json'])
    # Fixed adversarial distinctions: checks of authored examples, not runtime scorers.
    code = {c['case_id']: c for c in cases if c['category'] == 'coding'}
    assert any(t['args'] == [[2, 2, 2]] and t['output'] == 3 for t in code['code-002']['expected']['tests'])
    assert any(t['args'] == ['Aa'] and t['output'] is False for t in code['code-005']['expected']['tests'])
    assert set(t['args'][0] for t in code['code-007']['expected']['tests']) == set(range(13))
    assert len({c['case_id'] for c in cases}) == 60
    print(json.dumps({'cases': len(cases), 'python_assertions': sum(len(c['expected']['tests']) for c in code.values()), 'manifest_sha256': digest(files['manifest.json'].encode()), 'mode': 'write_new' if args.write_new else 'reproducibility_check'}, indent=2))


if __name__ == '__main__':
    main()
