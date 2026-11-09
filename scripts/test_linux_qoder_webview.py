"""Offline harness checks are not real GUI acceptance."""
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch
import linux_qoder_webview as subject

class Tests(unittest.TestCase):
    def test_private_fixtures_and_no_write_snapshots(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); paths = subject.fixtures(root)
            before = subject.snapshot(root)
            self.assertEqual(set(paths), {'unknown', 'incomplete', 'conflict'})
            self.assertEqual(list(paths['unknown'].iterdir()), [])
            for directory in paths.values():
                self.assertEqual(directory.stat().st_mode & 0o777, 0o700)
                for path in directory.iterdir(): self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertEqual(before, subject.snapshot(root))
            (paths['incomplete'] / 'archive.tar.gz').write_bytes(b'changed')
            self.assertNotEqual(before, subject.snapshot(root))

    def test_guard_rejects_non_ci_or_sandbox_override(self):
        with patch.dict(os.environ, {}, clear=True), self.assertRaises(subject.Failure):
            subject.guarded_run(Path('/unused'),Path('/unused'),Path('/unused'),Path('/unused'))
        env={'GITHUB_ACTIONS':'true','RUNNER_ENVIRONMENT':'github-hosted','RUNNER_OS':'Linux','WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS':'1'}
        with patch.dict(os.environ, env, clear=True), self.assertRaises(subject.Failure):
            subject.guarded_run(Path('/unused'),Path('/unused'),Path('/unused'),Path('/unused'))

    def test_protocol_click_type_and_read_are_real_element_commands(self):
        driver=subject.Driver(); driver.session='session'
        driver.request=Mock(side_effect=[{subject.ELEMENT:'button'}])
        driver.elements=Mock(return_value=[{subject.ELEMENT:'control'}])
        driver.request=Mock(return_value=None)
        driver.click('//button'); driver.type('//input','text')
        calls=[c.args for c in driver.request.call_args_list]
        self.assertEqual(calls[0],('POST','/session/session/element/control/click',{}))
        self.assertEqual(calls[1],('POST','/session/session/element/control/clear',{}))
        self.assertEqual(calls[2],('POST','/session/session/element/control/value',{'text':'text','value':list('text')}))
        self.assertFalse(any('execute' in c[1] for c in calls))

    def test_ambiguous_control_or_script_fails(self):
        driver=subject.Driver(); driver.elements=Mock(return_value=[])
        with self.assertRaises(subject.Failure):driver.one('//button')
        driver.elements.return_value=[{},{}]
        with self.assertRaises(subject.Failure):driver.one('//button')
        with self.assertRaises(subject.Failure):driver.request('POST','/session/id/execute/sync',{})

    def test_all_redirects_and_http_failures_close_without_following(self):
        for code in (301, 302, 303, 307, 308, 400, 404, 500):
            driver=subject.Driver();driver.connection=Mock()
            response=Mock();response.chunked=False;response.length=0;response.status=code;response.read.return_value=b'{"value":null}'
            driver.connection.getresponse.return_value=response
            with self.assertRaises(subject.Failure):driver.request('POST','/session/id/element/id/click',{})
            driver.connection.request.assert_called_once()
            driver.connection.close.assert_called_once()
            response.close.assert_called_once()

    def test_response_size_bound(self):
        driver=subject.Driver();driver.connection=Mock()
        response=Mock();response.chunked=False;response.length=0;response.status=200;response.read.return_value=b'x'*1_048_577
        driver.connection.getresponse.return_value=response
        with self.assertRaises(subject.Failure):driver.request('GET','/status')
        response.read.assert_called_once_with(1_048_577)
        driver.connection.close.assert_called_once()

    def test_disconnected_click_is_not_replayed(self):
        import http.client
        driver=subject.Driver();driver.connection=Mock()
        driver.connection.getresponse.side_effect=http.client.RemoteDisconnected('SECRET')
        with self.assertRaises(http.client.RemoteDisconnected):driver.request('POST','/session/id/element/id/click',{})
        driver.connection.request.assert_called_once()
        driver.connection.close.assert_called_once()
        self.assertFalse(driver.last_response['received'])
        self.assertEqual(subject.exception_class(http.client.RemoteDisconnected('SECRET')),'RemoteDisconnected')
        self.assertEqual(subject.failure_code(http.client.RemoteDisconnected('SECRET')),'webdriver_transport_error')

    def test_real_http11_connection_reused_without_connection_close_header(self):
        import http.client
        from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
        import threading
        observed=[]
        class Handler(BaseHTTPRequestHandler):
            protocol_version='HTTP/1.1'
            def do_POST(self):
                body=self.rfile.read(int(self.headers['Content-Length']))
                observed.append((self.client_address,dict(self.headers),body))
                result=b'{"value":null}'
                self.send_response(200);self.send_header('Content-Length',str(len(result)));self.end_headers()
                self.wfile.write(result);self.wfile.flush()
            def log_message(self,*args):pass
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        real=http.client.HTTPConnection
        try:
            with patch.object(subject.http.client,'HTTPConnection',side_effect=lambda host,port,timeout:real(host,server.server_port,timeout=timeout)) as factory:
                driver=subject.Driver()
                factory.assert_called_once_with('127.0.0.1',4444,timeout=30)
                try:
                    self.assertIsNone(driver.request('POST','/session/id/element/id/click',{}))
                    self.assertIsNone(driver.request('POST','/session/id/element/id/clear',{}))
                finally:driver.close()
            self.assertEqual(len(observed),2)
            self.assertEqual(observed[0][0],observed[1][0])
            for _,headers,body in observed:
                self.assertNotIn('Connection',headers)
                self.assertEqual(int(headers['Content-Length']),len(body))
                self.assertNotIn('Transfer-Encoding',headers)
        finally:
            server.shutdown();server.server_close();thread.join(timeout=2)

    def test_real_short_content_length_frame_is_rejected_without_replay(self):
        import http.client
        from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
        import threading
        received=[]
        class Handler(BaseHTTPRequestHandler):
            protocol_version='HTTP/1.1'
            def do_POST(self):
                received.append(self.path)
                self.rfile.read(int(self.headers['Content-Length']))
                self.send_response(200)
                self.send_header('Content-Length','100')
                self.end_headers()
                self.wfile.write(b'{"value":null}'); self.wfile.flush()
                self.close_connection=True
            def log_message(self,*args):pass
        server=ThreadingHTTPServer(('127.0.0.1',0),Handler)
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        real=http.client.HTTPConnection
        try:
            with patch.object(subject.http.client,'HTTPConnection',side_effect=lambda host,port,timeout:real(host,server.server_port,timeout=timeout)):
                driver=subject.Driver()
                try:
                    with self.assertRaisesRegex(subject.Failure,'driver_incomplete_response'):
                        driver.request('POST','/session/id/element/id/click',{})
                    self.assertEqual(received,['/session/id/element/id/click'])
                    self.assertIsNone(driver.connection.sock)
                finally:driver.close()
        finally:
            server.shutdown();server.server_close();thread.join(timeout=2)

    def test_log_capture_is_bounded_but_drains(self):
        stream=io.BytesIO(b'x'*2_000_000); output=bytearray()
        subject.capture_log(stream,output)
        self.assertEqual(len(output),1_048_576);self.assertEqual(stream.tell(),2_000_000)

    def test_exercise_asserts_all_three_states_and_fails_on_mutation(self):
        class Fake:
            def __init__(self, paths, mutate=False):
                self.paths=paths; self.kind=None; self.visible=False; self.current_value=''; self.mutate=mutate; self.inspect_clicks=0
            def elements(self, xpath):
                if '缓存诊断结果' in xpath: return [{}] if self.visible else []
                return [{}]
            def wait(self, predicate, seconds=20):
                if not predicate(): raise subject.Failure('predicate_failed')
            def type(self, xpath, text):
                self.current_value=text;self.visible=False
                self.kind=next(k for k,p in self.paths.items() if str(p)==text)
            def click(self, xpath):
                if '只读诊断' in xpath:
                    self.inspect_clicks += 1
                    self.visible=True
                    if self.mutate: (self.paths[self.kind]/'new').write_text('changed')
                if '清除' in xpath:self.visible=False;self.current_value=''
            def text(self, xpath):
                return subject.LABELS[self.kind] + ' 本报告不授权删除'
            def one(self, xpath):return 'control'
            def command(self, *args):return self.current_value
            def value(self, xpath):return self.current_value
            def enabled(self, xpath):return True
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);paths=subject.fixtures(root);records=[]
            phases=[]
            subject.exercise(Fake(paths),paths,root,records,phases.append)
            self.assertLess(phases.index('unknown_confirm_typed_path'),phases.index('unknown_click_inspect'))
            self.assertLess(phases.index('unknown_wait_inspect_enabled'),phases.index('unknown_click_inspect'))
            self.assertEqual([r['state'] for r in records],list(subject.LABELS))
            startup=Fake(paths); original_elements=startup.elements
            startup.elements=lambda xpath: [] if 'workspace-tab-updates' in xpath else original_elements(xpath)
            with self.assertRaisesRegex(subject.Failure,'startup_registry_not_idle'):subject.exercise(startup,paths,root,[])
            self.assertEqual(startup.inspect_clicks,0)
            wrong_value=Fake(paths);wrong_value.value=lambda xpath:'wrong'
            with self.assertRaises(subject.Failure):subject.exercise(wrong_value,paths,root,[])
            self.assertEqual(wrong_value.inspect_clicks,0)
            disabled=Fake(paths);disabled.enabled=lambda xpath:False
            with self.assertRaises(subject.Failure):subject.exercise(disabled,paths,root,[])
            self.assertEqual(disabled.inspect_clicks,0)
            with self.assertRaises(subject.Failure):subject.exercise(Fake(paths,True),paths,root,[])

    def test_failure_dom_is_bounded_and_canary_checked_before_truncation(self):
        with tempfile.TemporaryDirectory() as tmp:
            driver=Mock();driver.session='session';driver.value.return_value='';driver.expected_input='';driver.enabled.return_value=True
            driver.text.return_value='visible application text'
            driver.command.return_value='x'*70000 + subject.CANARY
            subject.capture_failure_dom(driver,Path(tmp))
            report=json.loads((Path(tmp)/'failure-dom.json').read_text())
            self.assertEqual(report['body']['text'],'visible application text')
            self.assertEqual(report['source']['state'],'private_content_suppressed')
            driver.command.assert_called_once_with('GET','/source')
            driver.command.return_value='x'*70000
            subject.capture_failure_dom(driver,Path(tmp))
            report=json.loads((Path(tmp)/'failure-dom.json').read_text())
            self.assertTrue(report['source']['truncated'])
            self.assertEqual(len(report['source']['text']),65536)

    def test_failure_codes_never_include_raw_exception_or_backend_messages(self):
        with tempfile.TemporaryDirectory() as tmp:
            driver=Mock();driver.session='session';driver.value.return_value='';driver.expected_input='';driver.enabled.return_value=True
            driver.text.side_effect=ValueError('SECRET private message')
            driver.command.side_effect=subject.Failure('SECRET backend error')
            subject.capture_failure_dom(driver,Path(tmp))
            text=(Path(tmp)/'failure-dom.json').read_text()
            self.assertNotIn('SECRET',text)
            self.assertIn('unexpected_harness_error',text)
            self.assertEqual(subject.failure_code(subject.Failure('ui_wait_expired')),'ui_wait_expired')
        driver=Mock();driver.session=None
        with tempfile.TemporaryDirectory() as tmp:
            subject.capture_failure_dom(driver,Path(tmp));driver.text.assert_not_called();driver.command.assert_not_called()

    def test_w3c_null_response_and_malformed_bodies_are_distinguished(self):
        class Response(io.BytesIO):
            status=200
            chunked=False
            length=0
        for raw, expected in [(b'{"value":null}',None),(b'',json.JSONDecodeError),(b'[]',subject.Failure),(b'{"SECRET":"private"}',subject.Failure)]:
            driver=subject.Driver()
            driver.connection=Mock();driver.connection.getresponse.return_value=Response(raw)
            if expected is None:
                self.assertIsNone(driver.request('POST','/session/id/element/id/clear',{}))
            else:
                with self.assertRaises(expected):driver.request('POST','/session/id/element/id/clear',{})
                driver.connection.close.assert_called_once()
            self.assertEqual(driver.last_response['operation'],'clear')
            self.assertNotIn('SECRET',json.dumps(driver.last_response))
            self.assertNotIn('private',json.dumps(driver.last_response))
            self.assertEqual(driver.last_response['empty'],raw==b'')

    def test_type_records_the_exact_failing_substep_without_changing_sendkeys(self):
        driver=subject.Driver();driver.one=Mock(return_value='element')
        driver.command=Mock(side_effect=json.JSONDecodeError('SECRET','',0))
        with self.assertRaises(json.JSONDecodeError):driver.type('//input','private path')
        self.assertEqual(driver.input_stage,'clear_input')
        driver.command=Mock(side_effect=[None,TypeError('SECRET')])
        with self.assertRaises(TypeError):driver.type('//input','private path')
        self.assertEqual(driver.input_stage,'send_keys')
        self.assertEqual(subject.exception_class(TypeError('SECRET')),'TypeError')
        self.assertEqual(subject.exception_class(json.JSONDecodeError('SECRET','',0)),'JSONDecodeError')
        class SecretException(Exception): pass
        self.assertEqual(subject.exception_class(SecretException('private')),'OtherException')
        driver.command.assert_called_with('POST','/element/element/value',{'text':'private path','value':list('private path')})

    def test_control_diagnostics_do_not_dump_untrusted_input(self):
        with tempfile.TemporaryDirectory() as tmp:
            driver=Mock();driver.session='session';driver.expected_input='/expected'
            driver.value.return_value='SECRET-unexpected-input'
            driver.enabled.return_value=False
            driver.text.return_value='safe body';driver.command.return_value='safe source'
            subject.capture_failure_dom(driver,Path(tmp))
            text=(Path(tmp)/'failure-dom.json').read_text();report=json.loads(text)
            self.assertNotIn('SECRET',text)
            self.assertFalse(report['cache_input']['matches_expected'])
            self.assertFalse(report['inspect_button']['enabled'])

    def test_oversize_or_symlink_fixture_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);subject.fixtures(root)
            with (root/'oversize').open('wb') as f:f.truncate(65537)
            with self.assertRaises(subject.Failure):subject.snapshot(root)

if __name__=='__main__':unittest.main()
