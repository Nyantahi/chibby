import { Routes, Route } from 'react-router-dom';
import Layout from './components/Layout';
import Projects from './components/Projects';
import ProjectDetail from './components/ProjectDetail';
import AddProject from './components/AddProject';
import RunDetail from './components/RunDetail';
import Settings from './components/Settings';
import Templates from './components/Templates';
// TODO: Re-enable when Agent feature is complete (see private/plans)
// import AgentChat from './components/AgentChat';

function App() {
  return (
    <>
      <Routes>
        <Route path="/" element={<Layout />}>
          <Route index element={<Projects />} />
          <Route path="projects" element={<Projects />} />
          <Route path="project/:projectId" element={<ProjectDetail />} />
          <Route path="add-project" element={<AddProject />} />
          <Route path="run/:runId" element={<RunDetail />} />
          <Route path="templates" element={<Templates />} />
          <Route path="settings" element={<Settings />} />
        </Route>
      </Routes>
      {/* TODO: Re-enable Agent Chat when feature is complete (see private/plans) */}
      {/* <AgentChat /> */}
    </>
  );
}

export default App;
